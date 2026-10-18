//! Engine-owned services the Engine port calls back into. C++ fills this
//! table as a constant (`HP2EngineServices.cpp`) and passes a pointer on
//! every export call.

use std::ffi::c_void;

/// `#[repr(C)]` mirror of the callback table. Field order is ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2EngineServices {
    /// `appFrand()`: the engine float RNG (shared sequence, never reimplemented).
    pub app_frand: unsafe extern "C" fn() -> f32,
    /// `appRand()`: the engine integer RNG (shared sequence, never reimplemented).
    pub app_rand: unsafe extern "C" fn() -> i32,
    /// `ULevel::FarMoveActor(actor, dest, 0, 0)` on the level behind `level`.
    pub far_move_actor:
        unsafe extern "C" fn(level: *mut c_void, actor: *mut c_void, dest: *const c_void) -> i32,
    /// `APawn::pointReachable(point)` on `pawn`.
    pub point_reachable: unsafe extern "C" fn(pawn: *mut c_void, point: *const c_void) -> i32,
    /// `AActor::SetCollisionSize(radius, height)`.
    pub set_collision_size: unsafe extern "C" fn(actor: *mut c_void, radius: f32, height: f32),
    /// `UModel::FastLineCheck(end, start)`: returns the BYTE result.
    pub model_fast_line_check:
        unsafe extern "C" fn(model: *mut c_void, end: *const c_void, start: *const c_void) -> u8,
    /// `FRotator::Vector()`: writes the FVector result to `out`.
    pub rotator_vector: unsafe extern "C" fn(rot: *const c_void, out: *mut c_void),
}
