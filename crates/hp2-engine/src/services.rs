//! Engine-owned services the Engine port calls back into. C++ fills this
//! table as a constant (`HP2EngineServices.cpp`) and passes a pointer on
//! every export call.

use std::ffi::c_void;

/// `#[repr(C)]` mirror of the callback table. Field order is ABI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2EngineServices {
    /// `appFrand()`: the engine float RNG (shared sequence, never reimplemented).
    pub app_frand: unsafe extern "C" fn(out: *mut f32) -> i32,
    /// `appRand()`: the engine integer RNG (shared sequence, never reimplemented).
    pub app_rand: unsafe extern "C" fn(out: *mut i32) -> i32,
    /// `ULevel::FarMoveActor(actor, dest, 0, 0)` on the level behind `level`.
    pub far_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        dest: *const c_void,
        out: *mut i32,
    ) -> i32,
    /// `APawn::pointReachable(aPoint, 0)`.
    pub point_reachable:
        unsafe extern "C" fn(pawn: *mut c_void, point: *const c_void, out: *mut i32) -> i32,
    /// `UModel::FastLineCheck(end, start)`: returns the BYTE result.
    pub model_fast_line_check:
        unsafe extern "C" fn(model: *mut c_void, end: *const c_void, start: *const c_void) -> u8,
    /// `FRotator::Vector()`: writes the FVector result to `out`.
    pub rotator_vector: unsafe extern "C" fn(rot: *const c_void, out: *mut c_void),
    /// Native registry reads are terminal leaves; the loop rereads its live length.
    pub wind_count: unsafe extern "C" fn() -> i32,
    pub wind_at: unsafe extern "C" fn(i: i32) -> *mut c_void,
    /// Native process-global permutation storage, borrowed only between callbacks.
    pub wind_random: unsafe extern "C" fn() -> *mut c_void,
    /// Preserve the nested public native guards for reach-spec policy calls.
    pub reach_define_for:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut i32) -> i32,
    pub reach_find_best: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut c_void,
        *mut i32,
    ) -> i32,
}

static mut SERVICES: Option<Hp2EngineServices> = None;

/// Publish the process-wide `Hp2EngineServices` table once. Mirrors
/// `hp2_core::vm_object::publish_services`.
///
/// # Safety
/// `value` is the real, immutable `GHp2EngineServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2EngineServices) {
    unsafe { (&raw mut SERVICES).write(Some(value)) }
}

/// Read back the published table; `Err(1)` when nothing has published yet.
///
/// # Safety
/// Call only after engine startup has had a chance to publish.
pub unsafe fn published_services() -> Result<Hp2EngineServices, i32> {
    unsafe { (&raw const SERVICES).read().ok_or(1) }
}

/// Captured native exception; the enclosing native public entry rethrows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineThrown(pub i32);

#[inline]
pub fn checked(status: i32) -> Result<(), EngineThrown> {
    if status == 0 {
        Ok(())
    } else {
        Err(EngineThrown(status))
    }
}
