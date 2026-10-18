//! FFI exports for the sparks slice: spark spawning and editing, the per-frame
//! `RedrawSparks` and `PostDrawSparks` update, and the `FirePaint` and
//! `DrawSparkLine` editor handlers.

use hp2_ffi_support::guard;
use hp2_fire::abi::{FireSpark, FireTraceHooks};
use hp2_fire::sparks::{self, FireSparkState};

/// Views a raw array as a mutable slice. Callers null-check `ptr` first.
unsafe fn slice_mut<'a, T>(ptr: *mut T, len: usize) -> &'a mut [T] {
    unsafe { std::slice::from_raw_parts_mut(ptr, len) }
}

/// `UFireTexture::AddSpark`: spawns one spark of the current type at the mouse position.
/// `sparks_len` is `Sparks.Num()`, which equals `SparksLimit`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_add_spark(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
    mouse_x: i32,
    mouse_y: i32,
) {
    guard("hp2_rs_fire_add_spark", (), move || unsafe {
        if state.is_null() || sparks.is_null() {
            return;
        }
        sparks::add_spark(
            trace,
            &mut *state,
            slice_mut(sparks, sparks_len),
            mouse_x,
            mouse_y,
        );
    })
}

/// `UFireTexture::CloseSpark`: closes the most recent open lightning bolt.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_close_spark(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
) {
    guard("hp2_rs_fire_close_spark", (), move || unsafe {
        if state.is_null() || sparks.is_null() {
            return;
        }
        sparks::close_spark(trace, &mut *state, slice_mut(sparks, sparks_len));
    })
}

/// `UFireTexture::DeleteSparks`: removes sparks inside a diamond around the cursor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_delete_sparks(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
    spark_x: i32,
    spark_y: i32,
    area_width: i32,
) {
    guard("hp2_rs_fire_delete_sparks", (), move || unsafe {
        if state.is_null() || sparks.is_null() {
            return;
        }
        sparks::delete_sparks(
            trace,
            &mut *state,
            slice_mut(sparks, sparks_len),
            spark_x,
            spark_y,
            area_width,
        );
    })
}

/// `UFireTexture::DrawSparkLine`: Bresenham line of sparks between two points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_draw_spark_line(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
    start_x: i32,
    start_y: i32,
    dest_x: i32,
    dest_y: i32,
) {
    guard("hp2_rs_fire_draw_spark_line", (), move || unsafe {
        if state.is_null() || sparks.is_null() {
            return;
        }
        sparks::draw_spark_line(
            trace,
            &mut *state,
            slice_mut(sparks, sparks_len),
            start_x,
            start_y,
            dest_x,
            dest_y,
        );
    })
}

/// `UFireTexture::FirePaint`: the fire-specific editor mouse handler.
/// `buttons` is the raw `DWORD` mouse-button mask.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_fire_paint(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
    mouse_x: i32,
    mouse_y: i32,
    buttons: u32,
) {
    guard("hp2_rs_fire_fire_paint", (), move || unsafe {
        if state.is_null() || sparks.is_null() {
            return;
        }
        sparks::fire_paint(
            trace,
            &mut *state,
            slice_mut(sparks, sparks_len),
            mouse_x,
            mouse_y,
            buttons,
        );
    })
}

/// `UFireTexture::RedrawSparks`: advances every spark one frame, writing into `mips0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_redraw_sparks(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
    mips0: *mut u8,
    mips0_len: usize,
) {
    guard("hp2_rs_fire_redraw_sparks", (), move || unsafe {
        if state.is_null() || sparks.is_null() || mips0.is_null() {
            return;
        }
        sparks::redraw_sparks(
            trace,
            &mut *state,
            slice_mut(sparks, sparks_len),
            slice_mut(mips0, mips0_len),
        );
    })
}

/// `UFireTexture::PostDrawSparks`: restores each fixed star's pixel after the update.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_fire_post_draw_sparks(
    trace: FireTraceHooks,
    state: *mut FireSparkState,
    sparks: *mut FireSpark,
    sparks_len: usize,
    mips0: *mut u8,
    mips0_len: usize,
) {
    guard("hp2_rs_fire_post_draw_sparks", (), move || unsafe {
        if state.is_null() || sparks.is_null() || mips0.is_null() {
            return;
        }
        sparks::post_draw_sparks(
            trace,
            &mut *state,
            slice_mut(sparks, sparks_len),
            slice_mut(mips0, mips0_len),
        );
    })
}
