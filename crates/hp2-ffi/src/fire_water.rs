//! FFI exports for the water port (`crates/hp2-fire/src/water.rs`). Raw pointers
//! become slices here and nowhere else.

use hp2_ffi_support::guard;
use hp2_fire::abi::{FireDrop, FireTraceHooks};
use hp2_fire::water::FireWaterState;

/// # Safety
/// A non-null `ptr` must be valid for reads and writes of `len` elements for `'a`,
/// and must not be aliased by any other live reference.
unsafe fn slice_mut<'a, T>(ptr: *mut T, len: usize) -> Option<&'a mut [T]> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// # Safety
/// A non-null `ptr` must be valid for reads of `len` elements for `'a`.
unsafe fn slice_ref<'a, T>(ptr: *const T, len: usize) -> Option<&'a [T]> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Replaces `UWaterTexture::CalculateWater`.
///
/// # Safety
/// Each non-null pointer must be valid for its length. `bitmap`, `water`, `render`
/// and `wave` must not alias each other.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_calculate_water(
    trace: FireTraceHooks,
    st: *mut FireWaterState,
    bitmap: *mut u8,
    bitmap_len: usize,
    water: *mut u8,
    water_len: usize,
    render: *const u8,
    render_len: usize,
    wave: *const u8,
    wave_len: usize,
) {
    guard("hp2_rs_fire_calculate_water", (), move || unsafe {
        let (Some(st), Some(bitmap), Some(water), Some(render), Some(wave)) = (
            st.as_mut(),
            slice_mut(bitmap, bitmap_len),
            slice_mut(water, water_len),
            slice_ref(render, render_len),
            slice_ref(wave, wave_len),
        ) else {
            return;
        };
        hp2_fire::water::calculate_water(trace, st, bitmap, water, render, wave);
    });
}

/// Replaces `UWaterTexture::WaterRedrawDrops`.
///
/// # Safety
/// Each non-null pointer must be valid for its length. `drops` and `water` must not alias.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_water_redraw_drops(
    trace: FireTraceHooks,
    st: *mut FireWaterState,
    drops: *mut FireDrop,
    drops_len: usize,
    water: *mut u8,
    water_len: usize,
) {
    guard("hp2_rs_fire_water_redraw_drops", (), move || unsafe {
        let (Some(st), Some(drops), Some(water)) = (
            st.as_mut(),
            slice_mut(drops, drops_len),
            slice_mut(water, water_len),
        ) else {
            return;
        };
        hp2_fire::water::water_redraw_drops(trace, st, drops, water);
    });
}

/// Replaces `UWaterTexture::AddDrop`.
///
/// # Safety
/// `st` must be null or point to a live `FireWaterState`. `drops` must be null or
/// valid for `drops_len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_add_drop(
    trace: FireTraceHooks,
    st: *mut FireWaterState,
    drops: *mut FireDrop,
    drops_len: usize,
    drop_x: i32,
    drop_y: i32,
) {
    guard("hp2_rs_fire_add_drop", (), move || unsafe {
        let (Some(st), Some(drops)) = (st.as_mut(), slice_mut(drops, drops_len)) else {
            return;
        };
        hp2_fire::water::add_drop(trace, st, drops, drop_x, drop_y);
    });
}

/// Replaces `UWaterTexture::DeleteDrops`.
///
/// # Safety
/// `st` must be null or point to a live `FireWaterState`. `drops` must be null or
/// valid for `drops_len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_delete_drops(
    trace: FireTraceHooks,
    st: *mut FireWaterState,
    drops: *mut FireDrop,
    drops_len: usize,
    drop_x: i32,
    drop_y: i32,
    area_width: i32,
) {
    guard("hp2_rs_fire_delete_drops", (), move || unsafe {
        let (Some(st), Some(drops)) = (st.as_mut(), slice_mut(drops, drops_len)) else {
            return;
        };
        hp2_fire::water::delete_drops(trace, st, drops, drop_x, drop_y, area_width);
    });
}

/// Replaces `UWaterTexture::WaterPaint`.
///
/// # Safety
/// `st` must be null or point to a live `FireWaterState`. `drops` must be null or
/// valid for `drops_len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_water_paint(
    trace: FireTraceHooks,
    st: *mut FireWaterState,
    drops: *mut FireDrop,
    drops_len: usize,
    x: i32,
    y: i32,
    buttons: u32,
) {
    guard("hp2_rs_fire_water_paint", (), move || unsafe {
        let (Some(st), Some(drops)) = (st.as_mut(), slice_mut(drops, drops_len)) else {
            return;
        };
        hp2_fire::water::water_paint(trace, st, drops, x, y, buttons);
    });
}

/// Replaces `UWaterTexture::TouchTexture`.
///
/// # Safety
/// `st` must be null or point to a live `FireWaterState`. `water` must be null or
/// valid for `water_len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_touch_texture(
    trace: FireTraceHooks,
    st: *const FireWaterState,
    water: *mut u8,
    water_len: usize,
    u_pos: i32,
    v_pos: i32,
    magnitude: f32,
) {
    guard("hp2_rs_fire_touch_texture", (), move || unsafe {
        let (Some(st), Some(water)) = (st.as_ref(), slice_mut(water, water_len)) else {
            return;
        };
        hp2_fire::water::touch_texture(trace, st, water, u_pos, v_pos, magnitude);
    });
}

/// Replaces `UFireTexture::TempDrawSpark`.
///
/// # Safety
/// `bitmap` must be null or valid for `bitmap_len` elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_temp_draw_spark(
    trace: FireTraceHooks,
    bitmap: *mut u8,
    bitmap_len: usize,
    u_mask: i32,
    v_mask: i32,
    u_bits: u8,
    pos_x: i32,
    pos_y: i32,
    intensity: i32,
) {
    guard("hp2_rs_fire_temp_draw_spark", (), move || unsafe {
        let Some(bitmap) = slice_mut(bitmap, bitmap_len) else {
            return;
        };
        hp2_fire::water::temp_draw_spark(
            trace, bitmap, u_mask, v_mask, u_bits, pos_x, pos_y, intensity,
        );
    });
}
