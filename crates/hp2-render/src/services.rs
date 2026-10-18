//! Engine-owned services the Render port calls back into. C++ fills this
//! table once (`Render.cpp`) and passes a pointer on every export call.

use std::ffi::c_void;

/// `#[repr(C)]` mirror of the callback table. Field order is ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RenderServices {
    /// `appFrand()`: the engine RNG (shared sequence, never reimplemented).
    pub frand: unsafe extern "C" fn() -> f32,
    /// `FMemStack::PushBytes(size, align)` on the stack behind `mem`.
    pub mem_push: unsafe extern "C" fn(mem: *mut c_void, size: i32, align: i32) -> *mut u8,
    /// Placement `new(mark) FMemMark(*mem)`.
    pub mem_mark_save: unsafe extern "C" fn(mem: *mut c_void, mark: *mut c_void),
    /// `FMemMark::Pop()` on a mark previously saved by `mem_mark_save`.
    pub mem_mark_pop: unsafe extern "C" fn(mark: *mut c_void),
    /// `debugf(NAME_Warning, ...)` with a UTF-8 message.
    pub log_warning: unsafe extern "C" fn(text: *const u8, len: usize),
    /// `GMath.SinTab(angle)`: public accessor into the engine trig table.
    pub sin_tab: unsafe extern "C" fn(angle: i32) -> f32,
    /// `GMath.CosTab(angle)`.
    pub cos_tab: unsafe extern "C" fn(angle: i32) -> f32,
    /// Render.h `SqrtApprox(f)` (bare inline: `appSqrt` on this target).
    pub sqrt_approx: unsafe extern "C" fn(f: f32) -> f32,
    /// Render.h `DivSqrtApprox(f)` (bare inline).
    pub div_sqrt_approx: unsafe extern "C" fn(f: f32) -> f32,
    /// Render.h `DivApprox(f)` (bare inline: `1.0/f`).
    pub div_approx: unsafe extern "C" fn(f: f32) -> f32,
    /// `GMath.DivSqrtApprox(f)`: the table-based approximation that
    /// `ScreenVector` uses. Not the Render.h inline above.
    pub gmath_div_sqrt_approx: unsafe extern "C" fn(f: f32) -> f32,
    /// `GMath.SqrtApprox(f)`: the UnMath.h table approximation used by
    /// `FVector::SizeApprox` and `Pitch`. Not the Render.h inline above.
    pub gmath_sqrt_approx: unsafe extern "C" fn(f: f32) -> f32,
}
