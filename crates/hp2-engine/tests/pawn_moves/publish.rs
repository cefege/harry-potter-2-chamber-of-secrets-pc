//! Publishes the process-wide `Hp2PawnReachServices` table for consumer tests
//! whose code under test now calls the real Rust `APawn::walkMove` /
//! `APawn::jumpLanding` (`hp2_engine::pawn`, `hp2_engine::pawn_reach`).
//!
//! Each test binary scripts the pawn's collision world through `move_actor` and
//! `far_move_actor`; those callbacks read the binary's thread-local fixture, so
//! one table serves every parallel test. `with_hit` lends a fresh stack
//! `FCheckResult` like the C++ constructor (`Time = 1`, `Item = INDEX_NONE`).
//! The walk/landing bodies reach no other service; each of the rest fails with
//! status 1 if called.

use std::ffi::c_void;
use std::sync::{Once, OnceLock};

use hp2_abi::engine::generated::FCheckResult;
use hp2_engine::pawn::{Hp2PawnServices, PawnHitBody};
use hp2_engine::pawn_reach::{self, Hp2PawnReachServices};

pub type MoveActor = unsafe extern "C" fn(
    *mut c_void,
    *mut c_void,
    *const c_void,
    *const c_void,
    *mut c_void,
    *mut i32,
) -> i32;
pub type FarMoveActor =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut i32) -> i32;

/// Publish once per process; the first caller's callbacks win, so a binary
/// must always pass the same pair.
pub fn publish(move_actor: MoveActor, far_move_actor: FarMoveActor) {
    static TABLE: OnceLock<Hp2PawnReachServices> = OnceLock::new();
    static PUBLISHED: Once = Once::new();
    PUBLISHED.call_once(|| {
        let table = TABLE.get_or_init(|| Hp2PawnReachServices {
            pawn: Hp2PawnServices {
                move_actor,
                far_move_actor,
                find_water_line: unused_water,
                with_hit,
            },
            far_move_actor_flags: unused_far_flags,
            single_line_check: unused_trace,
            point_region: unused_region,
            fast_line_check: unused_fast,
            is_a: unused_is_a,
            is_editor: unused_editor,
        });
        // SAFETY: published once, before any reader in this process.
        unsafe { pawn_reach::publish_services(table) };
    });
}

unsafe extern "C" fn unused_water(_: *mut c_void, _: *const c_void, _: *mut c_void) -> i32 {
    1
}
unsafe extern "C" fn with_hit(context: *mut c_void, body: PawnHitBody) -> i32 {
    // SAFETY: all-zero is a valid `FCheckResult` (null pointers, zero floats).
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = 1.0;
    hit.item = -1;
    // SAFETY: `body` only touches the hit for the duration of this call.
    unsafe { body(context, (&raw mut hit).cast()) }
}
unsafe extern "C" fn unused_far_flags(
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: i32,
    _: i32,
    _: *mut i32,
) -> i32 {
    1
}
unsafe extern "C" fn unused_trace(
    _: *mut c_void,
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: *const c_void,
) -> i32 {
    1
}
unsafe extern "C" fn unused_region(
    _: *mut c_void,
    _: *mut c_void,
    _: *const c_void,
    _: *mut c_void,
) -> i32 {
    1
}
unsafe extern "C" fn unused_fast(
    _: *mut c_void,
    _: *const c_void,
    _: *const c_void,
    _: *mut i32,
) -> i32 {
    1
}
unsafe extern "C" fn unused_is_a(_: *mut c_void, _: i32, _: *mut i32) -> i32 {
    1
}
unsafe extern "C" fn unused_editor(_: *mut i32) -> i32 {
    1
}
