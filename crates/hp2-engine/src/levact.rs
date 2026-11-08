//! Level-actor helpers from `Engine/Src/UnLevAct.cpp`: the rotation-delta
//! functions used by CheckEncroachment and the mover multi-line gate.
//!
//! FMA: the original object (clang -O3 on arm64, `-ffp-contract=on`) fuses
//! `A*A + B*B` into `fma(A, A, B*B)`, and each rotation step into
//! `fma(c, d, -(p*q))` for the cos/sin product difference and into
//! `fma(length, inner, origin)` for the offset. The Y/Z sum is
//! `fma(sa, cb, sb*ca)`. These sites are mirrored with `mul_add` below and
//! pinned by `levact_rotation.txt`.

use std::ffi::{CStr, c_char, c_void};
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, APawn, FCheckResult, FMemStack, FPointRegion, FRotator, TArrayRaw, ULevel,
};
use hp2_abi::render_span::FMemMark;
use hp2_core::mem;

use crate::actor;
use crate::math::app_sqrt;
use crate::native_guard::guarded;
use crate::prim::{INDEX_NONE, empty_result};
use crate::vec::{add, div, scale, size, size_squared, sub, zero};

/// `ME_*` values of `EMoverEncroachType` (`EngineClasses.h`).
pub const ME_STOP_WHEN_ENCROACH: u8 = 0;
pub const ME_RETURN_WHEN_ENCROACH: u8 = 1;
pub const ME_CRUSH_WHEN_ENCROACH: u8 = 2;
pub const ME_IGNORE_WHEN_ENCROACH: u8 = 3;

/// `appSin(angle / (32768.0f / PI))` and `appCos(...)`: the float argument
/// is widened to DOUBLE and each result narrowed to FLOAT.
fn alpha_sin_cos(angle: f32) -> (f32, f32) {
    // `32768.0f / PI` folds in float; `PI` is `3.1415926535897932f`.
    let k: f32 = 32768.0_f32 / std::f32::consts::PI;
    let arg = f64::from(angle / k);
    (arg.sin() as f32, arg.cos() as f32)
}

/// Shared body of the three single-axis rotations: rotates the in-plane
/// offset `(a, b)` by `angle` and returns the two rotated components, each
/// offset by its origin component `o1`/`o2`.
fn rotate_plane(a: f32, b: f32, angle: f32, o1: f32, o2: f32) -> (f32, f32) {
    let lsq = a.mul_add(a, b * b);
    let length = app_sqrt(f64::from(lsq)) as f32;
    let cos_betha = a / length;
    let sin_betha = b / length;
    let (sin_alpha, cos_alpha) = alpha_sin_cos(angle);
    let inner1 = cos_alpha.mul_add(cos_betha, -(sin_alpha * sin_betha));
    let inner2 = sin_alpha.mul_add(cos_betha, sin_betha * cos_alpha);
    (length.mul_add(inner1, o1), length.mul_add(inner2, o2))
}

/// `FindDeltaFromRotationYaw(origin, point, angle)`: rotates `point` about
/// the Z axis through `origin`.
pub fn find_delta_from_rotation_yaw(origin: FVector, point: FVector, angle: i32) -> FVector {
    let (x, y) = rotate_plane(
        point.x - origin.x,
        point.y - origin.y,
        angle as f32,
        origin.x,
        origin.y,
    );
    FVector { x, y, z: point.z }
}

/// `FindDeltaFromRotationRoll(origin, point, angle)`: rotates `point` about
/// the X axis through `origin`. The angle is negated (orientation).
pub fn find_delta_from_rotation_roll(origin: FVector, point: FVector, angle: i32) -> FVector {
    let (y, z) = rotate_plane(
        point.y - origin.y,
        point.z - origin.z,
        -(angle as f32),
        origin.y,
        origin.z,
    );
    FVector { x: point.x, y, z }
}

/// `FindDeltaFromRotationPitch(origin, point, angle)`: rotates `point` about
/// the Y axis through `origin`.
pub fn find_delta_from_rotation_pitch(origin: FVector, point: FVector, angle: i32) -> FVector {
    let (x, z) = rotate_plane(
        point.x - origin.x,
        point.z - origin.z,
        angle as f32,
        origin.x,
        origin.z,
    );
    FVector { x, y: point.y, z }
}

/// `FindDeltaFromRotation`: roll, then pitch, then yaw; returns the
/// displacement from `point`.
pub fn find_delta_from_rotation(
    origin: FVector,
    point: FVector,
    yaw: i32,
    roll: i32,
    pitch: i32,
) -> FVector {
    let v1 = find_delta_from_rotation_roll(origin, point, roll);
    let v2 = find_delta_from_rotation_pitch(origin, v1, pitch);
    let v3 = find_delta_from_rotation_yaw(origin, v2, yaw);
    FVector {
        x: v3.x - point.x,
        y: v3.y - point.y,
        z: v3.z - point.z,
    }
}

/// `DoMultiLineCheck(Actor)`. `is_mover` is `Actor->IsA(AMover::StaticClass())`
/// and `encroach_type` is `Mover->MoverEncroachType`, read only when it is a
/// mover. Non-movers return `true`; movers return `true` for the four ME_*
/// encroach modes and `false` otherwise.
pub fn do_multi_line_check(is_mover: bool, encroach_type: u8) -> bool {
    if !is_mover {
        return true;
    }
    matches!(
        encroach_type,
        ME_STOP_WHEN_ENCROACH
            | ME_RETURN_WHEN_ENCROACH
            | ME_CRUSH_WHEN_ENCROACH
            | ME_IGNORE_WHEN_ENCROACH
    )
}

/// `ACTOR` kinds for `LevActServices::actor_is_a` (ALevelInfo, AMover, AZoneInfo).
pub const ACTOR_KIND_LEVEL_INFO: i32 = 0;
pub const ACTOR_KIND_MOVER: i32 = 1;
pub const ACTOR_KIND_ZONE_INFO: i32 = 2;
pub const ACTOR_KIND_BRUSH: i32 = 3;
pub const ACTOR_KIND_PAWN: i32 = 4;
pub const ACTOR_KIND_DECORATION: i32 = 5;

/// `TRACE_*` flags (`UnLevel.h`).
pub const TRACE_BLOCK_ACTORS: u32 = 0x01;
pub const TRACE_MOVERS: u32 = 0x02;
pub const TRACE_LEVEL: u32 = 0x04;
pub const TRACE_ZONE_CHANGES: u32 = 0x08;
pub const TRACE_OTHERS: u32 = 0x10;
/// `TRACE_AllColliding`.
pub const TRACE_ALL_COLLIDING: u32 = TRACE_BLOCK_ACTORS | TRACE_MOVERS | TRACE_LEVEL | TRACE_OTHERS;
/// `TRACE_AllBlocking` (`TRACE_BlockActors | TRACE_Level | TRACE_Movers`).
pub const TRACE_ALL_BLOCKING: u32 = TRACE_BLOCK_ACTORS | TRACE_LEVEL | TRACE_MOVERS;



/// C++ services (`Hp2LevActServices` in `UnLevAct.cpp`), same field order.
/// Actor and level handles are `void*`; the C++ thunks do the owner calls.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LevActServices {
    /// `new(mem, count) FCheckResult`, returned uninitialized.
    pub alloc_check_results:
        unsafe extern "C" fn(mem: *mut c_void, count: i32, result: *mut *mut FCheckResult) -> i32,
    /// `Level->GetLevel()->Model->PointCheck(*hit, NULL, loc, ext, 0)`.
    pub model_point_check: unsafe extern "C" fn(
        level: *mut c_void,
        hit: *mut FCheckResult,
        location: *const FVector,
        extent: *const FVector,
        result: *mut i32,
    ) -> i32,
    /// `Level->GetLevel()->Model->LineCheck(*hit, NULL, end, start, ext, flags)`.
    pub model_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        flags: u32,
        result: *mut i32,
    ) -> i32,
    /// `Hash->ActorPointCheck(*mem, loc, ext, flags)`.
    pub hash_actor_point_check: unsafe extern "C" fn(
        hash: *mut c_void,
        mem: *mut c_void,
        location: *const FVector,
        extent: *const FVector,
        flags: u32,
        result: *mut *mut FCheckResult,
    ) -> i32,
    /// `Hash->ActorLineCheck(*mem, end, start, ext, flags)`.
    pub hash_actor_line_check: unsafe extern "C" fn(
        hash: *mut c_void,
        mem: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        flags: u32,
        result: *mut *mut FCheckResult,
    ) -> i32,
    /// `Actor->IsA(kind class)`.
    pub actor_is_a: unsafe extern "C" fn(actor: *const c_void, kind: i32, result: *mut i32) -> i32,
    /// `Source->IsOwnedBy(Other)`.
    pub actor_is_owned_by:
        unsafe extern "C" fn(actor: *const c_void, other: *const c_void, result: *mut i32) -> i32,
    /// `Source->IsBlockedBy(Other)`.
    pub actor_is_blocked_by:
        unsafe extern "C" fn(actor: *const c_void, other: *const c_void, result: *mut i32) -> i32,
    /// `appFailAssert(expr, "UnLevAct.cpp", line)`.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    pub app_cycles: unsafe extern "C" fn(result: *mut u32) -> i32,
    pub is_editor: unsafe extern "C" fn(result: *mut i32) -> i32,
    pub bump_observer: unsafe extern "C" fn(recipient: *mut c_void, other: *mut c_void) -> i32,
    pub sin_tab: unsafe extern "C" fn(angle: i32) -> f32,
    pub cos_tab: unsafe extern "C" fn(angle: i32) -> f32,
    pub model_point_region: unsafe extern "C" fn(
        model: *mut c_void,
        zone: *mut c_void,
        location: *const FVector,
        out: *mut FPointRegion,
    ) -> i32,
    pub hash_add_actor: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void) -> i32,
    pub hash_remove_actor: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void) -> i32,
    pub hash_actor_encroachment_check: unsafe extern "C" fn(
        hash: *mut c_void,
        mem: *mut c_void,
        actor: *mut c_void,
        location: *const FVector,
        rotation: *const FRotator,
        flags: u32,
        result: *mut *mut FCheckResult,
    ) -> i32,
    pub tracker_update: unsafe extern "C" fn(tracker: *mut c_void, actor: *mut c_void) -> i32,
    pub actor_begin_touch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub actor_end_touch:
        unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, no_notify: i32) -> i32,
    pub actor_is_overlapping:
        unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, result: *mut i32) -> i32,
    pub actor_collision_box:
        unsafe extern "C" fn(actor: *mut c_void, min: *mut FVector, max: *mut FVector) -> i32,
    pub actor_move_smooth:
        unsafe extern "C" fn(actor: *mut c_void, delta: *const FVector, result: *mut i32) -> i32,
    pub event_bump: unsafe extern "C" fn(recipient: *mut c_void, other: *mut c_void) -> i32,
    pub event_encroaching_on:
        unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, result: *mut i32) -> i32,
    pub event_encroached_by: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub event_zone_change: unsafe extern "C" fn(actor: *mut c_void, zone: *mut c_void) -> i32,
    pub zone_event_actor_leaving:
        unsafe extern "C" fn(zone: *mut c_void, actor: *mut c_void) -> i32,
    pub zone_event_actor_entered:
        unsafe extern "C" fn(zone: *mut c_void, actor: *mut c_void) -> i32,
    pub pawn_event_foot_zone_change:
        unsafe extern "C" fn(pawn: *mut c_void, zone: *mut c_void) -> i32,
    pub pawn_event_head_zone_change:
        unsafe extern "C" fn(pawn: *mut c_void, zone: *mut c_void) -> i32,
    pub pri_set_player_zone: unsafe extern "C" fn(pri: *mut c_void, zone: *mut c_void) -> i32,
    pub level_net_mode: unsafe extern "C" fn(level_info: *mut c_void, result: *mut u8) -> i32,
    /// Virtual-dispatch leaves: each does a real `((ULevel*)Level)->Method(...)`
    /// call so a C++ override (e.g. a test fixture subclassing `ULevel`) is
    /// honored, instead of a direct Rust-to-Rust hub call. Used only by the
    /// handful of call sites where the original C++ body itself called
    /// another virtual method through `this`/`GetLevel()`.
    pub virtual_set_actor_zone: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        test: i32,
        force_refresh: i32,
    ) -> i32,
    pub virtual_multi_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        mem: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        check_actors: i32,
        level_info: *mut c_void,
        extra_node_flags: u8,
        result: *mut *mut FCheckResult,
    ) -> i32,
    pub virtual_multi_point_check: unsafe extern "C" fn(
        level: *mut c_void,
        mem: *mut c_void,
        location: *const FVector,
        extent: *const FVector,
        extra_node_flags: u32,
        level_info: *mut c_void,
        check_actors: i32,
        result: *mut *mut FCheckResult,
    ) -> i32,
    pub virtual_single_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        hit: *mut FCheckResult,
        source: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        trace_flags: u32,
        extent: *const FVector,
        extra_node_flags: u8,
        result: *mut i32,
    ) -> i32,
    pub virtual_single_point_check: unsafe extern "C" fn(
        level: *mut c_void,
        hit: *mut FCheckResult,
        location: *const FVector,
        extent: *const FVector,
        extra_node_flags: u32,
        level_info: *mut c_void,
        check_actors: i32,
        result: *mut i32,
    ) -> i32,
    pub virtual_check_encroachment: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        test_location: *const FVector,
        test_rotation: *const FRotator,
        touch_notify: i32,
        result: *mut i32,
    ) -> i32,
    pub virtual_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        delta: *const FVector,
        new_rotation: *const FRotator,
        hit: *mut FCheckResult,
        test: i32,
        ignore_pawns: i32,
        ignore_bases: i32,
        no_fail: i32,
        result: *mut i32,
    ) -> i32,
    pub virtual_far_move_actor: unsafe extern "C" fn(
        level: *mut c_void,
        actor: *mut c_void,
        destination: *const FVector,
        test: i32,
        no_check: i32,
        result: *mut i32,
    ) -> i32,
    pub virtual_find_spot: unsafe extern "C" fn(
        level: *mut c_void,
        extent: *const FVector,
        location: *mut FVector,
        check_actors: i32,
        assume_fit: i32,
        result: *mut i32,
    ) -> i32,
    pub virtual_adjust_spot: unsafe extern "C" fn(
        level: *mut c_void,
        adjusted: *mut FVector,
        trace_dest: *const FVector,
        trace_len: f32,
        hit: *mut FCheckResult,
    ) -> i32,
}

impl LevActServices {
    unsafe fn alloc_check_results(
        &self,
        mem: *mut c_void,
        count: i32,
    ) -> Result<*mut FCheckResult, i32> {
        let mut result = std::mem::MaybeUninit::<*mut FCheckResult>::uninit();
        let status = unsafe { (self.alloc_check_results)(mem, count, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn model_point_check(
        &self,
        level: *mut c_void,
        hit: *mut FCheckResult,
        location: *const FVector,
        extent: *const FVector,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status =
            unsafe { (self.model_point_check)(level, hit, location, extent, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn model_line_check(
        &self,
        level: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        flags: u32,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.model_line_check)(level, hit, end, start, extent, flags, result.as_mut_ptr())
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn hash_actor_point_check(
        &self,
        hash: *mut c_void,
        mem: *mut c_void,
        location: *const FVector,
        extent: *const FVector,
        flags: u32,
    ) -> Result<*mut FCheckResult, i32> {
        let mut result = std::mem::MaybeUninit::<*mut FCheckResult>::uninit();
        let status = unsafe {
            (self.hash_actor_point_check)(hash, mem, location, extent, flags, result.as_mut_ptr())
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn hash_actor_line_check(
        &self,
        hash: *mut c_void,
        mem: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        flags: u32,
    ) -> Result<*mut FCheckResult, i32> {
        let mut result = std::mem::MaybeUninit::<*mut FCheckResult>::uninit();
        let status = unsafe {
            (self.hash_actor_line_check)(hash, mem, end, start, extent, flags, result.as_mut_ptr())
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn actor_is_a(&self, actor: *const c_void, kind: i32) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_is_a)(actor, kind, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn actor_is_owned_by(
        &self,
        actor: *const c_void,
        other: *const c_void,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_is_owned_by)(actor, other, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn actor_is_blocked_by(
        &self,
        actor: *const c_void,
        other: *const c_void,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_is_blocked_by)(actor, other, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn check_failed(&self, expr: *const c_char, line: i32) -> Result<(), i32> {
        let status = unsafe { (self.check_failed)(expr, line) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn app_cycles(&self) -> Result<u32, i32> {
        let mut result = std::mem::MaybeUninit::<u32>::uninit();
        let status = unsafe { (self.app_cycles)(result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    pub(crate) unsafe fn is_editor(&self) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.is_editor)(result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn bump_observer(&self, recipient: *mut c_void, other: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.bump_observer)(recipient, other) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn model_point_region(
        &self,
        model: *mut c_void,
        zone: *mut c_void,
        location: *const FVector,
        out: *mut FPointRegion,
    ) -> Result<(), i32> {
        let status = unsafe { (self.model_point_region)(model, zone, location, out) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn hash_add_actor(&self, hash: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.hash_add_actor)(hash, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn hash_remove_actor(&self, hash: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.hash_remove_actor)(hash, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn hash_actor_encroachment_check(
        &self,
        hash: *mut c_void,
        mem: *mut c_void,
        actor: *mut c_void,
        location: *const FVector,
        rotation: *const FRotator,
        flags: u32,
    ) -> Result<*mut FCheckResult, i32> {
        let mut result = std::mem::MaybeUninit::<*mut FCheckResult>::uninit();
        let status = unsafe {
            (self.hash_actor_encroachment_check)(
                hash,
                mem,
                actor,
                location,
                rotation,
                flags,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn tracker_update(&self, tracker: *mut c_void, actor: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.tracker_update)(tracker, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_begin_touch(&self, actor: *mut c_void, other: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.actor_begin_touch)(actor, other) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_end_touch(
        &self,
        actor: *mut c_void,
        other: *mut c_void,
        no_notify: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_end_touch)(actor, other, no_notify) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_is_overlapping(
        &self,
        actor: *mut c_void,
        other: *mut c_void,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_is_overlapping)(actor, other, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn actor_collision_box(
        &self,
        actor: *mut c_void,
        min: *mut FVector,
        max: *mut FVector,
    ) -> Result<(), i32> {
        let status = unsafe { (self.actor_collision_box)(actor, min, max) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn actor_move_smooth(
        &self,
        actor: *mut c_void,
        delta: *const FVector,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.actor_move_smooth)(actor, delta, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn event_bump(&self, recipient: *mut c_void, other: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.event_bump)(recipient, other) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn event_encroaching_on(
        &self,
        actor: *mut c_void,
        other: *mut c_void,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe { (self.event_encroaching_on)(actor, other, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    unsafe fn event_encroached_by(
        &self,
        actor: *mut c_void,
        other: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.event_encroached_by)(actor, other) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn event_zone_change(&self, actor: *mut c_void, zone: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.event_zone_change)(actor, zone) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn zone_event_actor_leaving(
        &self,
        zone: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.zone_event_actor_leaving)(zone, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn zone_event_actor_entered(
        &self,
        zone: *mut c_void,
        actor: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.zone_event_actor_entered)(zone, actor) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn pawn_event_foot_zone_change(
        &self,
        pawn: *mut c_void,
        zone: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.pawn_event_foot_zone_change)(pawn, zone) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn pawn_event_head_zone_change(
        &self,
        pawn: *mut c_void,
        zone: *mut c_void,
    ) -> Result<(), i32> {
        let status = unsafe { (self.pawn_event_head_zone_change)(pawn, zone) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn pri_set_player_zone(&self, pri: *mut c_void, zone: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.pri_set_player_zone)(pri, zone) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    unsafe fn level_net_mode(&self, level_info: *mut c_void) -> Result<u8, i32> {
        let mut result = std::mem::MaybeUninit::<u8>::uninit();
        let status = unsafe { (self.level_net_mode)(level_info, result.as_mut_ptr()) };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_set_actor_zone(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        test: i32,
        force_refresh: i32,
    ) -> Result<(), i32> {
        let status = unsafe { (self.virtual_set_actor_zone)(level, actor, test, force_refresh) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_multi_line_check(
        &self,
        level: *mut c_void,
        mem: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        check_actors: i32,
        level_info: *mut c_void,
        extra_node_flags: u8,
    ) -> Result<*mut FCheckResult, i32> {
        let mut result = std::mem::MaybeUninit::<*mut FCheckResult>::uninit();
        let status = unsafe {
            (self.virtual_multi_line_check)(
                level,
                mem,
                end,
                start,
                extent,
                check_actors,
                level_info,
                extra_node_flags,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_multi_point_check(
        &self,
        level: *mut c_void,
        mem: *mut c_void,
        location: *const FVector,
        extent: *const FVector,
        extra_node_flags: u32,
        level_info: *mut c_void,
        check_actors: i32,
    ) -> Result<*mut FCheckResult, i32> {
        let mut result = std::mem::MaybeUninit::<*mut FCheckResult>::uninit();
        let status = unsafe {
            (self.virtual_multi_point_check)(
                level,
                mem,
                location,
                extent,
                extra_node_flags,
                level_info,
                check_actors,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_single_line_check(
        &self,
        level: *mut c_void,
        hit: *mut FCheckResult,
        source: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        trace_flags: u32,
        extent: *const FVector,
        extra_node_flags: u8,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.virtual_single_line_check)(
                level,
                hit,
                source,
                end,
                start,
                trace_flags,
                extent,
                extra_node_flags,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_single_point_check(
        &self,
        level: *mut c_void,
        hit: *mut FCheckResult,
        location: *const FVector,
        extent: *const FVector,
        extra_node_flags: u32,
        level_info: *mut c_void,
        check_actors: i32,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.virtual_single_point_check)(
                level,
                hit,
                location,
                extent,
                extra_node_flags,
                level_info,
                check_actors,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_check_encroachment(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        test_location: *const FVector,
        test_rotation: *const FRotator,
        touch_notify: i32,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.virtual_check_encroachment)(
                level,
                actor,
                test_location,
                test_rotation,
                touch_notify,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_move_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        delta: *const FVector,
        new_rotation: *const FRotator,
        hit: *mut FCheckResult,
        test: i32,
        ignore_pawns: i32,
        ignore_bases: i32,
        no_fail: i32,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.virtual_move_actor)(
                level,
                actor,
                delta,
                new_rotation,
                hit,
                test,
                ignore_pawns,
                ignore_bases,
                no_fail,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_far_move_actor(
        &self,
        level: *mut c_void,
        actor: *mut c_void,
        destination: *const FVector,
        test: i32,
        no_check: i32,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.virtual_far_move_actor)(
                level,
                actor,
                destination,
                test,
                no_check,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_find_spot(
        &self,
        level: *mut c_void,
        extent: *const FVector,
        location: *mut FVector,
        check_actors: i32,
        assume_fit: i32,
    ) -> Result<i32, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.virtual_find_spot)(
                level,
                extent,
                location,
                check_actors,
                assume_fit,
                result.as_mut_ptr(),
            )
        };
        if status == 0 {
            Ok(unsafe { result.assume_init() })
        } else {
            Err(status)
        }
    }
    /// # Safety
    /// `level` and all pointer arguments must be live.
    pub unsafe fn virtual_adjust_spot(
        &self,
        level: *mut c_void,
        adjusted: *mut FVector,
        trace_dest: *const FVector,
        trace_len: f32,
        hit: *mut FCheckResult,
    ) -> Result<(), i32> {
        let status =
            unsafe { (self.virtual_adjust_spot)(level, adjusted, trace_dest, trace_len, hit) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}

/// Fails the call path with the C++ `check(Expr)` behavior at `line` when
/// `ok` is false, mirroring the shell this hub replaced.
pub(crate) fn check(svc: &LevActServices, ok: bool, expr: &CStr, line: i32) -> Result<(), i32> {
    if !ok {
        unsafe { svc.check_failed(expr.as_ptr(), line)? };
    }
    Ok(())
}

/// Services, GMem, and the live native ULevel. Hash is reread after callbacks.
#[derive(Clone, Copy)]
pub struct LevelCtx<'a> {
    pub svc: &'a LevActServices,
    pub gmem: *mut c_void,
    pub ulevel: *mut ULevel,
}

/// Stored once by [`publish_services`]; lets other slices' leaves call the
/// levact hubs directly instead of round-tripping through a `ULevel` shell,
/// and lets the C++ shells themselves stop passing `&GHp2LevActServices,
/// &GMem` on every call.
static mut LEVACT_SERVICES: Option<(&'static LevActServices, *mut c_void)> = None;

/// Publishes the process-lifetime `GHp2LevActServices` table and `&GMem`.
/// Called once from `UnLevAct.cpp`'s static initializer, mirroring
/// `crate::route::publish_services`/`crate::pawn_reach::publish_services`.
///
/// # Safety
/// `services` is the real, immutable `GHp2LevActServices` table and `gmem`
/// is the process-lifetime `GMem`. Publication happens once, before any
/// engine call, and invokes no callbacks.
pub unsafe fn publish_services(services: &'static LevActServices, gmem: *mut FMemStack) {
    unsafe { (&raw mut LEVACT_SERVICES).write(Some((services, gmem.cast()))) };
}

/// The table and `GMem` stored by [`publish_services`]; `Err(1)` before
/// publication.
///
/// # Safety
/// No concurrent [`publish_services`] call.
pub unsafe fn published_services() -> Result<(&'static LevActServices, *mut c_void), i32> {
    unsafe { (&raw const LEVACT_SERVICES).read() }.ok_or(1)
}

pub(crate) unsafe fn level_hash(ctx: LevelCtx<'_>) -> *mut c_void {
    unsafe { ptr::addr_of!((*ctx.ulevel).hash).read_unaligned() }
}

/// Line-check segment in C++ argument order: `End`, `Start`, `Extent`.
#[derive(Clone, Copy)]
pub struct LineQuery {
    pub end: FVector,
    pub start: FVector,
    pub extent: FVector,
}

/// `FCheckResult(InTime)`: Next and Actor NULL, zero Location and Normal,
/// Item `INDEX_NONE`. C++ `FCheckResult Hits[64]` is left uninitialized; only
/// slots that were written are read, so the zeroed default is not observable.
pub(crate) fn check_result(time: f32) -> FCheckResult {
    let mut r = empty_result();
    r.time = time;
    r.item = INDEX_NONE;
    r
}

/// `GetNext()` on a hit-list node.
///
/// # Safety
/// `p` must point to a live `FCheckResult`.
#[inline]
unsafe fn next_of(p: *const FCheckResult) -> *mut FCheckResult {
    unsafe { (*p).next.cast() }
}


/// `ULevel::MultiPointCheck(Mem, Location, Extent, ExtraNodeFlags, Level, bActors)`.
/// Owns the guard. The level hit is allocated from `GMem` even when `mem`
/// differs, as in C++ (`new(GMem)`); that is kept on purpose.
///
/// # Safety
/// `ctx`, `mem`, `level` (when non-null) and the services must be valid.
pub unsafe fn multi_point_check(
    ctx: LevelCtx,
    mem: *mut c_void,
    level: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_node_flags: u32,
    check_actors: bool,
) -> Result<*mut FCheckResult, i32> {
    unsafe {
        guarded(c"ULevel::MultiPointCheck", || {
            multi_point_check_body(
                ctx,
                mem,
                level,
                location,
                extent,
                extra_node_flags,
                check_actors,
            )
        })
    }
}

unsafe fn multi_point_check_body(
    ctx: LevelCtx,
    mem: *mut c_void,
    level: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_node_flags: u32,
    check_actors: bool,
) -> Result<*mut FCheckResult, i32> {
    unsafe {
        crate::level_trace_owner::multi_point_check_with_ops(
            &mut NativeMultiPointOps(ctx),
            mem,
            level,
            location,
            extent,
            extra_node_flags,
            check_actors,
        )
    }
}

/// `ULevel::SinglePointCheck(Hit, Location, Extent, ExtraNodeFlags, Level, bActors)`.
/// Owns the guard and the native `FMemMark` around the call. Returns `true` when
/// there is no hit (`return 1`).
///
/// # Safety
/// Same requirements as [`multi_point_check`]; `hit` must be valid.
pub unsafe fn single_point_check(
    ctx: LevelCtx,
    hit: *mut FCheckResult,
    level: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_node_flags: u32,
    check_actors: bool,
) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::SinglePointCheck", || {
            let mut mark = std::mem::MaybeUninit::<FMemMark>::uninit();
            mem::mark_init(mark.as_mut_ptr(), ctx.gmem.cast::<FMemStack>());
            let mark = mark.assume_init();
            let no_hit = single_point_check_body(
                ctx,
                hit,
                level,
                location,
                extent,
                extra_node_flags,
                check_actors,
            )?;
            mem::mark_pop(&mark);
            Ok(no_hit)
        })
    }
}

unsafe fn single_point_check_body(
    ctx: LevelCtx,
    hit: *mut FCheckResult,
    level: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_node_flags: u32,
    check_actors: bool,
) -> Result<bool, i32> {
    unsafe {
        let first = crate::level_trace_owner::multi_point_check_with_ops(
            &mut NativeMultiPointOps(ctx),
            ctx.gmem,
            level,
            location,
            extent,
            extra_node_flags,
            check_actors,
        )?;
        Ok(crate::level_trace_owner::select_point_hit(hit, first, location))
    }
}

/// `ULevel::MultiLineCheck(Mem, End, Start, Extent, bCheckActors, LevelInfo,
/// ExtraNodeFlags)`. The level hit goes in `hits[0]`, hash links fill the rest
/// (cap 64), mover-vs-world ties are adjusted, then the list is qsorted by
/// `Time` and copied into `mem` with `Next` chained. Owns the guard.
///
/// # Safety
/// Same requirements as [`multi_point_check`].
pub unsafe fn multi_line_check(
    ctx: LevelCtx,
    mem: *mut c_void,
    level: *mut c_void,
    seg: LineQuery,
    check_actors: bool,
    extra_node_flags: u32,
) -> Result<*mut FCheckResult, i32> {
    unsafe {
        guarded(c"ULevel::MultiLineCheck", || {
            multi_line_check_body(ctx, mem, level, seg, check_actors, extra_node_flags)
        })
    }
}

unsafe fn multi_line_check_body(
    ctx: LevelCtx,
    mem: *mut c_void,
    level: *mut c_void,
    seg: LineQuery,
    check_actors: bool,
    extra_node_flags: u32,
) -> Result<*mut FCheckResult, i32> {
    let LineQuery { end, start, extent } = seg;
    unsafe {
        crate::level_trace_owner::multi_line_check_with_ops(
            &mut NativeMultiLineOps(ctx),
            mem,
            level,
            end,
            start,
            extent,
            check_actors,
            extra_node_flags,
        )
    }
}

struct NativeMultiPointOps<'a>(LevelCtx<'a>);

impl crate::level_trace_owner::MultiPointOps for NativeMultiPointOps<'_> {
    type Error = i32;

    unsafe fn actor_point_check(
        &mut self,
        memory: *mut c_void,
        location: FVector,
        extent: FVector,
        extra_flags: u32,
    ) -> Result<*mut FCheckResult, i32> {
        let hash = unsafe { level_hash(self.0) };
        if hash.is_null() { return Ok(ptr::null_mut()); }
        self.0.svc.hash_actor_point_check(hash, memory, &location, &extent, extra_flags)
    }

    unsafe fn world_point_check(
        &mut self,
        level_info: *mut c_void,
        hit: *mut FCheckResult,
        location: FVector,
        extent: FVector,
        _extra_flags: u32,
    ) -> Result<bool, i32> {
        if level_info.is_null() { return Ok(true); }
        Ok(unsafe { self.0.svc.model_point_check(level_info, hit, &location, &extent)? != 0 })
    }

    unsafe fn alloc_result(
        &mut self,
        _memory: *mut c_void,
        result: FCheckResult,
    ) -> Result<*mut FCheckResult, i32> {
        let hit = self.0.svc.alloc_check_results(self.0.gmem, 1)?;
        unsafe { hit.write(result); }
        Ok(hit)
    }
}

struct NativeMultiLineOps<'a>(LevelCtx<'a>);

impl crate::level_trace_owner::MultiLineOps for NativeMultiLineOps<'_> {
    type Error = i32;

    unsafe fn world_line_check(
        &mut self,
        level_info: *mut c_void,
        hit: *mut FCheckResult,
        end: FVector,
        start: FVector,
        extent: FVector,
        flags: u32,
    ) -> Result<bool, i32> {
        if level_info.is_null() { return Ok(true); }
        Ok(unsafe { self.0.svc.model_line_check(level_info, hit, &end, &start, &extent, flags)? != 0 })
    }

    unsafe fn actor_line_check(
        &mut self,
        memory: *mut c_void,
        end: FVector,
        start: FVector,
        extent: FVector,
        flags: u32,
    ) -> Result<*mut FCheckResult, i32> {
        let hash = unsafe { level_hash(self.0) };
        if hash.is_null() { return Ok(ptr::null_mut()); }
        self.0.svc.hash_actor_line_check(hash, memory, &end, &start, &extent, flags)
    }

    unsafe fn is_mover(&mut self, actor: *mut AActor) -> Result<bool, i32> {
        Ok(unsafe { self.0.svc.actor_is_a(actor.cast(), ACTOR_KIND_MOVER)? != 0 })
    }

    unsafe fn alloc_results(&mut self, memory: *mut c_void, count: usize) -> Result<*mut FCheckResult, i32> {
        self.0.svc.alloc_check_results(memory, count as i32)
    }
}

/// `ULevel::SingleLineCheck(Hit, SourceActor, End, Start, TraceFlags, Extent,
/// ExtraNodeFlags)`. Owns the guard and the native `FMemMark` around the call;
/// resolves `level_info` as `GetLevelInfo()` only when `TRACE_Level` is set,
/// matching the C++ shell's ternary. Returns `true` when nothing blocks
/// (`return Check==NULL`).
///
/// # Safety
/// Same requirements as [`multi_point_check`]; `source` may be null.
pub unsafe fn single_line_check(
    ctx: LevelCtx,
    hit: *mut FCheckResult,
    source: *mut c_void,
    seg: LineQuery,
    trace_flags: u32,
    extra_node_flags: u8,
) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::Trace", || {
            let level_info_arg = if trace_flags & TRACE_LEVEL != 0 {
                level_info(ctx)
            } else {
                ptr::null_mut()
            };
            let mut mark = std::mem::MaybeUninit::<FMemMark>::uninit();
            mem::mark_init(mark.as_mut_ptr(), ctx.gmem.cast::<FMemStack>());
            let mark = mark.assume_init();
            let no_hit = single_line_check_body(
                ctx,
                hit,
                source,
                level_info_arg,
                seg,
                trace_flags,
                extra_node_flags,
            )?;
            mem::mark_pop(&mark);
            Ok(no_hit)
        })
    }
}

unsafe fn single_line_check_body(
    ctx: LevelCtx,
    hit: *mut FCheckResult,
    source: *mut c_void,
    level_info: *mut c_void,
    seg: LineQuery,
    trace_flags: u32,
    extra_node_flags: u8,
) -> Result<bool, i32> {
    unsafe {
        let first = crate::level_trace_owner::multi_line_check_with_ops(
            &mut NativeMultiLineOps(ctx),
            ctx.gmem,
            level_info,
            seg.end,
            seg.start,
            seg.extent,
            trace_flags & TRACE_ALL_COLLIDING != 0,
            u32::from(extra_node_flags),
        )?;
        crate::level_trace_owner::filter_line_hits_with_ops(
            &mut NativeLineFilterOps(ctx.svc),
            hit,
            source.cast(),
            first,
            trace_flags,
        )
    }
}

struct NativeLineFilterOps<'a>(&'a LevActServices);

impl crate::level_trace_owner::LineFilterOps for NativeLineFilterOps<'_> {
    type Error = i32;

    unsafe fn is_owned_by(&mut self, source: *mut AActor, other: *mut AActor) -> Result<bool, i32> {
        Ok(unsafe { self.0.actor_is_owned_by(source.cast(), other.cast())? != 0 })
    }

    unsafe fn is_kind(
        &mut self,
        actor: *mut AActor,
        kind: crate::level_trace_owner::LineActorKind,
    ) -> Result<bool, i32> {
        let kind = match kind {
            crate::level_trace_owner::LineActorKind::LevelInfo => ACTOR_KIND_LEVEL_INFO,
            crate::level_trace_owner::LineActorKind::Mover => ACTOR_KIND_MOVER,
            crate::level_trace_owner::LineActorKind::ZoneInfo => ACTOR_KIND_ZONE_INFO,
        };
        Ok(unsafe { self.0.actor_is_a(actor.cast(), kind)? != 0 })
    }

    unsafe fn is_blocked_by(&mut self, source: *mut AActor, other: *mut AActor) -> Result<bool, i32> {
        Ok(unsafe { self.0.actor_is_blocked_by(source.cast(), other.cast())? != 0 })
    }
}
/// Returns `ULevel::Actors(0)`, including the original unchecked empty-array case.
///
/// # Safety
/// `ctx.ulevel` must point to a live `ULevel` whose actor array follows its ABI layout.
unsafe fn level_info(ctx: LevelCtx<'_>) -> *mut c_void {
    unsafe {
        let array = ptr::addr_of!((*ctx.ulevel).base.actors)
            .cast::<TArrayRaw>()
            .read_unaligned();
        array.data.cast::<*mut c_void>().read_unaligned()
    }
}

/// `ULevel::AdjustSpot`.
///
/// # Safety
/// Context and service pointers must be live.
pub unsafe fn adjust_spot(
    ctx: LevelCtx<'_>,
    adjusted: *mut FVector,
    trace_dest: FVector,
    trace_len: f32,
    hit: *mut FCheckResult,
) -> Result<(), i32> {
    unsafe {
        let line_end = trace_dest;
        let line_start = *adjusted;
        let line_extent = zero();
        let _ = ctx.svc.virtual_single_line_check(
            ctx.ulevel.cast(),
            hit,
            ptr::null_mut(),
            &line_end,
            &line_start,
            TRACE_LEVEL | TRACE_MOVERS,
            &line_extent,
            0,
        )?;
        if (*hit).time < 1.0 {
            // `Hit.Normal * (1.05f - Hit.Time) * TraceLen`: two vector scalings, unfused.
            *adjusted = add(
                *adjusted,
                scale(scale((*hit).normal, 1.05 - (*hit).time), trace_len),
            );
        }
        Ok(())
    }
}

/// `ULevel::FindSpot`.
///
/// # Safety
/// Context and service pointers must be live.
pub unsafe fn find_spot(
    ctx: LevelCtx<'_>,
    extent: FVector,
    location: *mut FVector,
    check_actors: bool,
    assume_fit: bool,
) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::FindSpot", || {
            find_spot_body(ctx, extent, location, check_actors, assume_fit)
        })
    }
}

unsafe fn find_spot_body(
    ctx: LevelCtx<'_>,
    extent: FVector,
    location: *mut FVector,
    check_actors: bool,
    assume_fit: bool,
) -> Result<bool, i32> {
    let info = unsafe { level_info(ctx) };
    let mut hit = check_result(1.0);
    let point_check = |at: FVector, hit: *mut FCheckResult| unsafe {
        ctx.svc.virtual_single_point_check(
            ctx.ulevel.cast(),
            hit,
            &at,
            &extent,
            0,
            info,
            check_actors as i32,
        ).map(|r| r != 0)
    };
    let adjust = |adjusted: &mut FVector, destination: FVector, trace_len: f32, hit: *mut FCheckResult| unsafe {
        ctx.svc.virtual_adjust_spot(
            ctx.ulevel.cast(),
            adjusted,
            &destination,
            trace_len,
            hit,
        )
    };
    unsafe {
        find_spot_policy(
            extent,
            location,
            assume_fit,
            &mut hit,
            point_check,
            adjust,
        )
    }
}

/// Shared `ULevel::FindSpot` adjustment policy for native and canonical adapters.
///
/// `point_check` and `adjust` adapt each runtime's collision implementation;
/// ordering and all displacement arithmetic stay in this policy.
pub(crate) unsafe fn find_spot_policy<E>(
    extent: FVector,
    location: *mut FVector,
    assume_fit: bool,
    hit: *mut FCheckResult,
    mut point_check: impl FnMut(FVector, *mut FCheckResult) -> Result<bool, E>,
    mut adjust: impl FnMut(&mut FVector, FVector, f32, *mut FCheckResult) -> Result<(), E>,
) -> Result<bool, E> {
    unsafe {
        if extent.x == 0.0 && extent.y == 0.0 && extent.z == 0.0 {
            return point_check(*location, hit);
        }
        if assume_fit && point_check(*location, hit)? {
            return Ok(true);
        }
        let original = *location;
        let mut adjusted = original;
        let trace_len = size(extent) + 2.0;
        for i in [-1.0_f32, 1.0] {
            let dest_x = add(adjusted, FVector { x: i * extent.x, y: 0.0, z: 0.0 });
            adjust(&mut adjusted, dest_x, extent.x, hit)?;
            let dest_y = add(adjusted, FVector { x: 0.0, y: i * extent.y, z: 0.0 });
            adjust(&mut adjusted, dest_y, extent.y, hit)?;
            let dest_z = add(adjusted, FVector { x: 0.0, y: 0.0, z: i * extent.z });
            adjust(&mut adjusted, dest_z, extent.z, hit)?;
        }
        if point_check(adjusted, hit)? {
            *location = adjusted;
            return Ok(true);
        }
        for i in [-1.0_f32, 1.0] {
            for j in [-1.0_f32, 1.0] {
                for k in [-1.0_f32, 1.0] {
                    let destination = add(adjusted, FVector {
                        x: i * extent.x,
                        y: j * extent.y,
                        z: k * extent.z,
                    });
                    adjust(&mut adjusted, destination, trace_len, hit)?;
                }
            }
        }
        let displacement = sub(adjusted, original);
        if f64::from(size_squared(displacement)) > 1.5_f64 * f64::from(size_squared(extent)) {
            return Ok(false);
        }
        if point_check(adjusted, hit)? {
            *location = adjusted;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Shared ordering for Actor zone exit, change, storage, and entry transitions.
pub(crate) fn zone_transition_policy<E>(
    actor: *mut c_void,
    old_region: FPointRegion,
    new_region: FPointRegion,
    test: bool,
    mut leaving: impl FnMut(*mut c_void, *mut c_void) -> Result<(), E>,
    mut changing: impl FnMut(*mut c_void, *mut c_void) -> Result<(), E>,
    mut store: impl FnMut(FPointRegion),
    mut entered: impl FnMut(*mut c_void, *mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    if new_region.zone != old_region.zone {
        if !test {
            leaving(old_region.zone, actor)?;
            changing(actor, new_region.zone)?;
        }
        store(new_region);
        if !test {
            entered(new_region.zone, actor)?;
        }
    } else {
        store(new_region);
    }
    Ok(())
}

/// Shared Pawn foot/head region transition ordering for native and canonical runtimes.
pub(crate) fn update_region_policy<E>(
    old_region: FPointRegion,
    new_region: FPointRegion,
    test: bool,
    mut changed: impl FnMut(*mut c_void) -> Result<(), E>,
    mut store: impl FnMut(FPointRegion),
) -> Result<(), E> {
    if new_region.zone != old_region.zone && !test {
        changed(new_region.zone)?;
    }
    store(new_region);
    Ok(())
}

/// `ULevel::SetActorZone`. Owns the guard and the `check(Actor)` precondition.
///
/// # Safety
/// `actor`, `ctx.ulevel`, and every non-null engine service pointer must be live.
pub unsafe fn set_actor_zone(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    test: bool,
    force_refresh: bool,
) -> Result<(), i32> {
    unsafe {
        guarded(c"ULevel::SetActorZone", || {
            check(ctx.svc, !actor.is_null(), c"Actor", 1439)?;
            set_actor_zone_body(ctx, actor, test, force_refresh)
        })
    }
}

unsafe fn set_actor_zone_body(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    test: bool,
    force_refresh: bool,
) -> Result<(), i32> {
    Ok(unsafe {
        unsafe {
            let a = actor.cast::<AActor>();
            if (*a).b_delete_me() {
                return Ok(());
            }
            let info = level_info(ctx);
            let mut fallback: FPointRegion = std::mem::zeroed();
            fallback.zone = info;
            fallback.i_leaf = -1;
            if actor == info {
                ptr::addr_of_mut!((*a).region).write_unaligned(fallback);
                return Ok(());
            }
            let pawn = if ctx.svc.actor_is_a(actor, ACTOR_KIND_PAWN)? != 0 {
                actor.cast::<APawn>()
            } else {
                ptr::null_mut()
            };
            if force_refresh {
                ptr::addr_of_mut!((*a).region).write_unaligned(fallback);
                if !pawn.is_null() {
                    ptr::addr_of_mut!((*pawn).foot_region).write_unaligned(fallback);
                    ptr::addr_of_mut!((*pawn).head_region).write_unaligned(fallback);
                }
            }
            let actors = ptr::addr_of!((*ctx.ulevel).base.actors)
                .cast::<TArrayRaw>()
                .read_unaligned();
            let zone = if actors.num != 0 { info } else { actor };
            let model = ptr::addr_of!((*ctx.ulevel).model).read_unaligned();
            let location = ptr::addr_of!((*a).location).read_unaligned();
            let mut new_region = fallback;
            ctx.svc
                .model_point_region(model, zone, &location, &mut new_region)?;
            let old_region = ptr::addr_of!((*a).region).read_unaligned();
            zone_transition_policy(
                actor,
                old_region,
                new_region,
                test,
                |zone, actor| ctx.svc.zone_event_actor_leaving(zone, actor),
                |actor, zone| ctx.svc.event_zone_change(actor, zone),
                |region| ptr::addr_of_mut!((*a).region).write_unaligned(region),
                |zone, actor| ctx.svc.zone_event_actor_entered(zone, actor),
            )?;
            if !pawn.is_null() {
                let pawn_location = ptr::addr_of!((*a).location).read_unaligned();
                let height = ptr::addr_of!((*a).collision_height).read_unaligned();
                let foot = sub(
                    pawn_location,
                    FVector {
                        x: 0.0,
                        y: 0.0,
                        z: height,
                    },
                );
                let mut new_foot = fallback;
                ctx.svc
                    .model_point_region(model, info, &foot, &mut new_foot)?;
                let old_foot = ptr::addr_of!((*pawn).foot_region).read_unaligned();
                update_region_policy(
                    old_foot,
                    new_foot,
                    test,
                    |zone| ctx.svc.pawn_event_foot_zone_change(pawn.cast(), zone),
                    |region| ptr::addr_of_mut!((*pawn).foot_region).write_unaligned(region),
                )?;
                let head_location = ptr::addr_of!((*a).location).read_unaligned();
                let eye_height = ptr::addr_of!((*pawn).eye_height).read_unaligned();
                let head = add(
                    head_location,
                    FVector {
                        x: 0.0,
                        y: 0.0,
                        z: eye_height,
                    },
                );
                let mut new_head = fallback;
                ctx.svc
                    .model_point_region(model, info, &head, &mut new_head)?;
                let old_head = ptr::addr_of!((*pawn).head_region).read_unaligned();
                update_region_policy(
                    old_head,
                    new_head,
                    test,
                    |zone| ctx.svc.pawn_event_head_zone_change(pawn.cast(), zone),
                    |region| ptr::addr_of_mut!((*pawn).head_region).write_unaligned(region),
                )?;

                let pri = ptr::addr_of!((*pawn).player_replication_info).read_unaligned();
                if !pri.is_null() && ctx.svc.level_net_mode(info)? != 3 {
                    let current_region = ptr::addr_of!((*a).region).read_unaligned();
                    ctx.svc.pri_set_player_zone(pri, current_region.zone)?;
                }
            }
        }
    })
}

/// `AActor::IsMovingBrush()` as used by the level movement code.
///
/// # Safety
/// `actor` and the actor-kind service must be live.
unsafe fn is_moving_brush(ctx: LevelCtx<'_>, actor: *mut AActor) -> Result<bool, i32> {
    Ok(unsafe {
        unsafe {
            !ptr::addr_of!((*actor).brush).read_unaligned().is_null()
                && ctx.svc.actor_is_a(actor.cast(), ACTOR_KIND_BRUSH)? != 0
                && !(*actor).b_static()
        }
    })
}

/// Runtime-specific operations used by the shared `ULevel::CheckEncroachment`
/// source policy. The hit traversal, mover response, event order, and touch
/// notifications are common to the native shell and canonical Core owner.
pub(crate) trait CheckEncroachmentOps {
    type Error;
    type Mark;

    unsafe fn mark(&mut self) -> Self::Mark;
    unsafe fn pop(&mut self, mark: Self::Mark);
    unsafe fn has_hash(&mut self) -> bool;
    unsafe fn level_info(&mut self) -> Result<*mut AActor, Self::Error>;
    unsafe fn hash_check(
        &mut self,
        actor: *mut AActor,
        location: FVector,
        rotation: FRotator,
    ) -> Result<*mut FCheckResult, Self::Error>;
    unsafe fn is_moving_brush(&mut self, actor: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn is_blocked_by(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error>;
    unsafe fn move_smooth(&mut self, actor: *mut AActor, delta: FVector) -> Result<(), Self::Error>;
    unsafe fn encroaching_on(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error>;
    unsafe fn is_overlapping(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error>;
    unsafe fn end_touch(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
        no_notify_self: bool,
    ) -> Result<(), Self::Error>;
    unsafe fn begin_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn encroached_by(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error>;
}

/// Shared source-order kernel for `ULevel::CheckEncroachment`.
///
/// # Safety
/// All actor pointers and the supplied operations' runtime resources remain live
/// through callbacks; hash-result pointers remain valid until `pop`.
pub(crate) unsafe fn check_encroachment_policy<O: CheckEncroachmentOps>(
    ops: &mut O,
    actor: *mut AActor,
    test_location: FVector,
    test_rotation: FRotator,
    touch_notify: bool,
) -> Result<bool, O::Error> {
    unsafe {
        if !(*actor).b_collide_actors()
            && !(*actor).b_block_actors()
            && !(*actor).b_block_players()
            && !ops.is_moving_brush(actor)?
        {
            return Ok(false);
        }

        let mark = ops.mark();
        let outcome = (|| {
            let mut first = ptr::null_mut();
            if ops.has_hash() {
                first = ops.hash_check(actor, test_location, test_rotation)?;
            }
            let level_info = ops.level_info()?;
            let mut test = first;
            while !test.is_null() {
                let other = (*test).actor.cast::<AActor>();
                let mut no_process = false;
                if other.cast::<c_void>() != actor.cast()
                    && other.cast::<c_void>() != level_info.cast()
                    && ops.is_blocked_by(actor, other)?
                {
                    if ops.is_moving_brush(actor)? && !ops.is_moving_brush(other)? {
                        let actor_location = ptr::addr_of!((*actor).location).read_unaligned();
                        let other_location = ptr::addr_of!((*other).location).read_unaligned();
                        let actor_rotation = ptr::addr_of!((*actor).rotation).read_unaligned();
                        let delta_rotation = find_delta_from_rotation(
                            actor_location,
                            other_location,
                            test_rotation.yaw.wrapping_sub(actor_rotation.yaw),
                            test_rotation.roll.wrapping_sub(actor_rotation.roll),
                            test_rotation.pitch.wrapping_sub(actor_rotation.pitch),
                        );
                        let move_dir = add(
                            sub(test_location, actor_location),
                            scale(delta_rotation, 10.0),
                        );
                        let _ = ops.move_smooth(other, move_dir)?;

                        // Native source deliberately performs this recheck without a Hash-null guard.
                        let recheck = ops.hash_check(actor, test_location, test_rotation)?;
                        no_process = true;
                        let mut re = recheck;
                        while !re.is_null() {
                            if (*re).actor == other.cast() {
                                no_process = false;
                                break;
                            }
                            re = next_of(re);
                        }
                        if !no_process {
                            let real_location = ptr::addr_of!((*actor).location).read_unaligned();
                            ptr::addr_of_mut!((*actor).location).write_unaligned(test_location);
                            let reverse = scale(move_dir, -1.0);
                            let _ = ops.move_smooth(other, reverse)?;
                            ptr::addr_of_mut!((*actor).location).write_unaligned(real_location);
                        }
                    }
                    if !no_process && ops.encroaching_on(actor, other)? {
                        return Ok(true);
                    }
                }
                test = next_of(test);
            }
            if touch_notify {
                for index in 0..4 {
                    let touching = ptr::addr_of!((*actor).touching)
                        .cast::<*mut c_void>()
                        .add(index)
                        .read_unaligned()
                        .cast::<AActor>();
                    if !touching.is_null() && !ops.is_overlapping(actor, touching)? {
                        ops.end_touch(actor, touching, false)?;
                    }
                }
            }
            test = first;
            while !test.is_null() {
                let other = (*test).actor.cast::<AActor>();
                if other.cast::<c_void>() != actor.cast()
                    && other.cast::<c_void>() != level_info.cast()
                {
                    if ops.is_blocked_by(actor, other)? {
                        ops.encroached_by(other, actor)?;
                    } else if touch_notify {
                        ops.begin_touch(actor, other)?;
                    }
                }
                test = next_of(test);
            }
            Ok(false)
        })();
        ops.pop(mark);
        outcome
    }
}

struct NativeCheckEncroachmentOps<'a>(LevelCtx<'a>);

impl CheckEncroachmentOps for NativeCheckEncroachmentOps<'_> {
    type Error = i32;
    type Mark = FMemMark;

    unsafe fn mark(&mut self) -> Self::Mark {
        unsafe {
            let mut mark = std::mem::MaybeUninit::<FMemMark>::uninit();
            mem::mark_init(mark.as_mut_ptr(), self.0.gmem.cast::<FMemStack>());
            mark.assume_init()
        }
    }
    unsafe fn pop(&mut self, mark: Self::Mark) {
        unsafe { mem::mark_pop(&mark); }
    }
    unsafe fn has_hash(&mut self) -> bool {
        !unsafe { level_hash(self.0) }.is_null()
    }
    unsafe fn level_info(&mut self) -> Result<*mut AActor, Self::Error> {
        Ok(unsafe { level_info(self.0).cast() })
    }
    unsafe fn hash_check(
        &mut self,
        actor: *mut AActor,
        location: FVector,
        rotation: FRotator,
    ) -> Result<*mut FCheckResult, Self::Error> {
        let hash = unsafe { level_hash(self.0) };
        unsafe {
            self.0.svc.hash_actor_encroachment_check(
                hash,
                self.0.gmem,
                actor.cast(),
                &location,
                &rotation,
                0,
            )
        }
    }
    unsafe fn is_moving_brush(&mut self, actor: *mut AActor) -> Result<bool, Self::Error> {
        unsafe { is_moving_brush(self.0, actor) }
    }
    unsafe fn is_blocked_by(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error> {
        Ok(unsafe { self.0.svc.actor_is_blocked_by(actor.cast(), other.cast())? != 0 })
    }
    unsafe fn move_smooth(&mut self, actor: *mut AActor, delta: FVector) -> Result<(), Self::Error> {
        unsafe { self.0.svc.actor_move_smooth(actor.cast(), &delta)?; }
        Ok(())
    }
    unsafe fn encroaching_on(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error> {
        Ok(unsafe { self.0.svc.event_encroaching_on(actor.cast(), other.cast())? != 0 })
    }
    unsafe fn is_overlapping(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error> {
        Ok(unsafe { self.0.svc.actor_is_overlapping(actor.cast(), other.cast())? != 0 })
    }
    unsafe fn end_touch(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
        no_notify_self: bool,
    ) -> Result<(), Self::Error> {
        unsafe { self.0.svc.actor_end_touch(actor.cast(), other.cast(), i32::from(no_notify_self)) }
    }
    unsafe fn begin_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error> {
        unsafe { self.0.svc.actor_begin_touch(actor.cast(), other.cast()) }
    }
    unsafe fn encroached_by(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), Self::Error> {
        unsafe { self.0.svc.event_encroached_by(actor.cast(), other.cast()) }
    }
}
/// `ULevel::CheckEncroachment`. Native adapter for the shared policy.
///
/// # Safety
/// Actor, context, and engine-owned memory must be live. The native services
/// must remain valid for synchronous callbacks.
pub unsafe fn check_encroachment(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    test_location: FVector,
    test_rotation: FRotator,
    touch_notify: bool,
) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::CheckEncroachment", || {
            check(ctx.svc, !actor.is_null(), c"Actor", 1329)?;
            let mut ops = NativeCheckEncroachmentOps(ctx);
            check_encroachment_policy(
                &mut ops,
                actor.cast(),
                test_location,
                test_rotation,
                touch_notify,
            )
        })
    }
}

/// `ULevel::Actors(index)`.
///
/// # Safety
/// The live level's actor array must contain `index`.
pub(crate) unsafe fn actor_at(ctx: LevelCtx<'_>, index: usize) -> *mut c_void {
    unsafe {
        let array = ptr::addr_of!((*ctx.ulevel).base.actors)
            .cast::<TArrayRaw>()
            .read_unaligned();
        array.data.cast::<*mut c_void>().add(index).read_unaligned()
    }
}

/// `Actors.Num()`, read fresh at every call like the C++ loop conditions do.
///
/// # Safety
/// `ctx.ulevel` must be live.
pub(crate) unsafe fn actor_count(ctx: LevelCtx<'_>) -> usize {
    unsafe {
        ptr::addr_of!((*ctx.ulevel).base.actors)
            .cast::<TArrayRaw>()
            .read_unaligned()
            .num as usize
    }
}

/// `ULevel::Actors(index) = value`: a plain element write to the live
/// backing storage, not an `Add`/`Remove` (no reallocation, no
/// `TTransArray` bookkeeping), exactly like the C++ `Actors(i) = X`
/// assignments it replaces.
///
/// # Safety
/// The live level's actor array must contain `index`.
pub(crate) unsafe fn set_actor_at(ctx: LevelCtx<'_>, index: usize, value: *mut c_void) {
    unsafe {
        let array = ptr::addr_of!((*ctx.ulevel).base.actors)
            .cast::<TArrayRaw>()
            .read_unaligned();
        array.data.cast::<*mut c_void>().add(index).write_unaligned(value);
    }
}

/// `ULevel::FarMoveActor`. Owns the guard and the `check(Actor!=NULL)`
/// precondition.
///
/// # Safety
/// Actor, level, and engine services must be live.
pub unsafe fn far_move_actor(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    destination: FVector,
    test: bool,
    no_check: bool,
) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::FarMoveActor", || {
            check(ctx.svc, !actor.is_null(), c"Actor!=NULL", 1237)?;
            far_move_actor_body(ctx, actor, destination, test, no_check)
        })
    }
}

unsafe fn far_move_actor_body(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    destination: FVector,
    test: bool,
    no_check: bool,
) -> Result<bool, i32> {
    let mut ops = NativeFarMoveActorOps(ctx);
    unsafe {
        crate::level_move_owner::far_move_actor_with_ops(
            &mut ops, ctx.ulevel, actor.cast(), destination, test, no_check,
        )
    }
}

struct NativeFarMoveActorOps<'a>(LevelCtx<'a>);

impl crate::level_move_owner::FarMoveActorOps for NativeFarMoveActorOps<'_> {
    type Error = i32;

    unsafe fn is_editor(&mut self) -> Result<bool, i32> {
        Ok(unsafe { self.0.svc.is_editor()? != 0 })
    }
    unsafe fn has_hash(&mut self, level: *mut ULevel) -> Result<bool, i32> {
        let _ = level;
        Ok(!unsafe { level_hash(self.0) }.is_null())
    }
    unsafe fn actor_collision_extent(&mut self, actor: *mut AActor) -> Result<FVector, i32> {
        let mut extent = zero();
        unsafe {
            actor::get_cylinder_extent(&actor::published_services()?, actor.cast(), &mut extent)?;
        }
        Ok(extent)
    }
    unsafe fn find_spot(
        &mut self,
        level: *mut ULevel,
        extent: FVector,
        location: *mut FVector,
    ) -> Result<bool, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.0.svc.virtual_find_spot)(
                level.cast(), &extent, location, 0, 0, result.as_mut_ptr(),
            )
        };
        if status == 0 { Ok(unsafe { result.assume_init() } != 0) } else { Err(status) }
    }
    unsafe fn level_info(&mut self, level: *mut ULevel) -> Result<*mut AActor, i32> {
        let _ = level;
        Ok(unsafe { level_info(self.0) }.cast())
    }
    unsafe fn net_mode(&mut self, level_info: *mut AActor) -> Result<i32, i32> {
        Ok(i32::from(unsafe { self.0.svc.level_net_mode(level_info.cast())? }))
    }
    unsafe fn check_encroachment(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        location: FVector,
        rotation: FRotator,
    ) -> Result<bool, i32> {
        let mut result = std::mem::MaybeUninit::<i32>::uninit();
        let status = unsafe {
            (self.0.svc.virtual_check_encroachment)(
                level.cast(), actor.cast(), &location, &rotation, 1, result.as_mut_ptr(),
            )
        };
        if status == 0 { Ok(unsafe { result.assume_init() } == 0) } else { Err(status) }
    }
    unsafe fn actor_count(&mut self, level: *mut ULevel) -> Result<i32, i32> {
        let _ = level;
        Ok(unsafe { actor_count(self.0) as i32 })
    }
    unsafe fn actor_at(&mut self, level: *mut ULevel, index: i32) -> Result<*mut AActor, i32> {
        let _ = level;
        Ok(unsafe { actor_at(self.0, index as usize) }.cast())
    }
    unsafe fn set_base(&mut self, actor: *mut AActor, new_base: *mut AActor) -> Result<(), i32> {
        unsafe { actor::set_base(&actor::published_services()?, actor, new_base, true) }
    }
    unsafe fn hash_remove(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), i32> {
        let _ = level;
        unsafe { self.0.svc.hash_remove_actor(level_hash(self.0), actor.cast()) }
    }
    unsafe fn hash_add(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), i32> {
        let _ = level;
        unsafe { self.0.svc.hash_add_actor(level_hash(self.0), actor.cast()) }
    }
    unsafe fn tracker_update(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), i32> {
        let _ = level;
        let tracker = unsafe { (*self.0.ulevel).brush_tracker };
        unsafe { self.0.svc.tracker_update(tracker, actor.cast()) }
    }
    unsafe fn set_actor_zone(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        test: bool,
    ) -> Result<(), i32> {
        unsafe { self.0.svc.virtual_set_actor_zone(level.cast(), actor.cast(), i32::from(test), 0) }
    }
}

fn rot_eq(a: FRotator, b: FRotator) -> bool {
    a.pitch == b.pitch && a.yaw == b.yaw && a.roll == b.roll
}

fn nearly_zero(v: FVector) -> bool {
    v.x.abs() < 1.0e-4 && v.y.abs() < 1.0e-4 && v.z.abs() < 1.0e-4
}

/// `ULevel::MoveActor`. Owns the guard and the `check(Actor!=NULL)`
/// precondition.
///
/// # Safety
/// `actor`, context, actor arrays, and all service pointers used by the selected
/// branches must be live.
#[allow(clippy::too_many_arguments)]
pub unsafe fn move_actor(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    delta: FVector,
    new_rotation: FRotator,
    hit: *mut FCheckResult,
    test: bool,
    ignore_pawns: bool,
    ignore_bases: bool,
    no_fail: bool,
) -> Result<bool, i32> {
    unsafe {
        guarded(c"ULevel::MoveActor", || {
            check(ctx.svc, !actor.is_null(), c"Actor!=NULL", 1305)?;
            move_actor_body(
                ctx,
                actor,
                delta,
                new_rotation,
                hit,
                test,
                ignore_pawns,
                ignore_bases,
                no_fail,
            )
        })
    }
}

/// `ULevel::MoveActor` body. The early failure paths intentionally retain the
/// original unpopped mark behavior.
///
/// # Safety
/// Same requirements as [`move_actor`].
#[allow(clippy::too_many_arguments)]
unsafe fn move_actor_body(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    delta: FVector,
    new_rotation: FRotator,
    hit: *mut FCheckResult,
    test: bool,
    ignore_pawns: bool,
    ignore_bases: bool,
    no_fail: bool,
) -> Result<bool, i32> {
    let mut ops = NativeMoveActorOps(ctx);
    unsafe {
        crate::level_move_owner::move_actor_with_ops(
            &mut ops,
            ctx.ulevel,
            actor.cast(),
            delta,
            new_rotation,
            hit,
            test,
            ignore_pawns,
            ignore_bases,
            no_fail,
        )
    }
}
struct NativeMoveActorOps<'a>(LevelCtx<'a>);

impl crate::level_move_owner::MoveActorOps for NativeMoveActorOps<'_> {
    type Error = i32;
    type Mark = FMemMark;
    unsafe fn is_editor(&mut self) -> Result<bool, i32> { Ok(unsafe { self.0.svc.is_editor()? } != 0) }
    unsafe fn mark_push(&mut self) -> Result<FMemMark, i32> {
        let mut mark = std::mem::MaybeUninit::<FMemMark>::uninit();
        unsafe { mem::mark_init(mark.as_mut_ptr(), self.0.gmem.cast::<FMemStack>()); }
        Ok(unsafe { mark.assume_init() })
    }
    unsafe fn mark_pop(&mut self, mark: &FMemMark) -> Result<(), i32> { unsafe { mem::mark_pop(mark); } Ok(()) }
    unsafe fn app_cycles(&mut self) -> Result<u32, i32> { unsafe { self.0.svc.app_cycles() } }
    unsafe fn actor_collision_box(&mut self, actor: *mut AActor) -> Result<(FVector, FVector), i32> {
        let mut min = zero();
        let mut max = zero();
        unsafe { self.0.svc.actor_collision_box(actor.cast(), &mut min, &mut max)?; }
        Ok((min, max))
    }
    unsafe fn is_moving_brush(&mut self, actor: *mut AActor) -> Result<bool, i32> {
        unsafe { is_moving_brush(self.0, actor) }
    }
    unsafe fn has_hash(&mut self, level: *mut ULevel) -> Result<bool, i32> { Ok(!unsafe { (*level).hash }.is_null()) }
    unsafe fn is_kind(&mut self, actor: *mut AActor, kind: crate::level_move_owner::MoveActorKind) -> Result<bool, i32> {
        let id = match kind {
            crate::level_move_owner::MoveActorKind::Mover => ACTOR_KIND_MOVER,
            crate::level_move_owner::MoveActorKind::Pawn => ACTOR_KIND_PAWN,
            crate::level_move_owner::MoveActorKind::Decoration => ACTOR_KIND_DECORATION,
        };
        Ok(unsafe { self.0.svc.actor_is_a(actor.cast(), id)? != 0 })
    }
    unsafe fn multi_line_check(&mut self, level: *mut ULevel, _actor: *mut AActor, end: FVector, start: FVector, extent: FVector, check_actors: bool, level_info: *mut AActor) -> Result<*mut FCheckResult, i32> {
        unsafe { self.0.svc.virtual_multi_line_check(level.cast(), self.0.gmem, &end, &start, &extent, check_actors as i32, level_info.cast(), 0) }
    }
    unsafe fn is_blocked_by(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<bool, i32> {
        Ok(unsafe { self.0.svc.actor_is_blocked_by(actor.cast(), other.cast())? != 0 })
    }
    unsafe fn actor_count(&mut self, level: *mut ULevel) -> Result<i32, i32> { let _ = level; Ok(unsafe { actor_count(self.0) } as i32) }
    unsafe fn actor_at(&mut self, level: *mut ULevel, index: i32) -> Result<*mut AActor, i32> { let _ = level; Ok(unsafe { actor_at(self.0, index as usize) }.cast()) }
    unsafe fn sin_cos(&mut self, angle: i32) -> Result<(f32, f32), i32> { Ok(((self.0.svc.sin_tab)(angle), (self.0.svc.cos_tab)(angle))) }
    unsafe fn move_actor_recursive(&mut self, level: *mut ULevel, actor: *mut AActor, delta: FVector, rotation: FRotator, hit: *mut FCheckResult, test: bool, ignore_pawns: bool, ignore_bases: bool, no_fail: bool) -> Result<bool, i32> {
        Ok(unsafe { self.0.svc.virtual_move_actor(level.cast(), actor.cast(), &delta, &rotation, hit, test as i32, ignore_pawns as i32, ignore_bases as i32, no_fail as i32)? != 0 })
    }
    unsafe fn check_encroachment(&mut self, level: *mut ULevel, actor: *mut AActor, location: FVector, rotation: FRotator) -> Result<bool, i32> {
        Ok(unsafe { self.0.svc.virtual_check_encroachment(level.cast(), actor.cast(), &location, &rotation, 0)? != 0 })
    }
    unsafe fn hash_remove(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), i32> {
        let _ = level;
        unsafe { self.0.svc.hash_remove_actor(level_hash(self.0), actor.cast()) }
    }
    unsafe fn hash_add(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), i32> {
        let _ = level;
        unsafe { self.0.svc.hash_add_actor(level_hash(self.0), actor.cast()) }
    }
    unsafe fn bump_observer(&mut self, recipient: *mut AActor, other: *mut AActor) -> Result<(), i32> { unsafe { self.0.svc.bump_observer(recipient.cast(), other.cast()) } }
    unsafe fn event_bump(&mut self, recipient: *mut AActor, other: *mut AActor) -> Result<(), i32> { unsafe { self.0.svc.event_bump(recipient.cast(), other.cast()) } }
    unsafe fn begin_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<(), i32> { unsafe { self.0.svc.actor_begin_touch(actor.cast(), other.cast()) } }
    unsafe fn is_overlapping(&mut self, actor: *mut AActor, other: *mut AActor) -> Result<bool, i32> { Ok(unsafe { self.0.svc.actor_is_overlapping(actor.cast(), other.cast())? != 0 }) }
    unsafe fn end_touch(&mut self, actor: *mut AActor, other: *mut AActor, no_notify: bool) -> Result<(), i32> { unsafe { self.0.svc.actor_end_touch(actor.cast(), other.cast(), no_notify as i32) } }
    unsafe fn set_actor_zone(&mut self, level: *mut ULevel, actor: *mut AActor, test: bool) -> Result<(), i32> { unsafe { self.0.svc.virtual_set_actor_zone(level.cast(), actor.cast(), test as i32, 0) } }
    unsafe fn tracker_update(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), i32> {
        let tracker = unsafe { (*level).brush_tracker };
        unsafe { self.0.svc.tracker_update(tracker, actor.cast()) }
    }
}

/// Reuses the shared `AActor::IsBasedOn` policy.
unsafe fn is_based_on(actor: *mut c_void, other: *mut c_void) -> bool {
    unsafe { crate::actor::is_based_on(actor.cast(), other.cast()) }
}
