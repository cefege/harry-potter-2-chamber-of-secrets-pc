//! Level-actor helpers from `Engine/Src/UnLevAct.cpp`: the rotation-delta
//! functions used by CheckEncroachment and the mover multi-line gate.
//!
//! FMA: the original object (clang -O3 on arm64, `-ffp-contract=on`) fuses
//! `A*A + B*B` into `fma(A, A, B*B)`, and each rotation step into
//! `fma(c, d, -(p*q))` for the cos/sin product difference and into
//! `fma(length, inner, origin)` for the offset. The Y/Z sum is
//! `fma(sa, cb, sb*ca)`. These sites are mirrored with `mul_add` below and
//! pinned by `levact_rotation.txt`.

use std::ffi::{c_int, c_void};
use std::mem::size_of;
use std::ptr;

use hp2_abi::FVector;
use hp2_abi::engine::generated::FCheckResult;

use crate::math::app_sqrt;
use crate::prim::{INDEX_NONE, empty_result};
use crate::vec::{add, div, scale, size, size_squared, sub};

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

/// `TRACE_*` flags (`UnLevel.h`).
pub const TRACE_BLOCK_ACTORS: u32 = 0x01;
pub const TRACE_MOVERS: u32 = 0x02;
pub const TRACE_LEVEL: u32 = 0x04;
pub const TRACE_ZONE_CHANGES: u32 = 0x08;
pub const TRACE_OTHERS: u32 = 0x10;
/// `TRACE_AllColliding`.
pub const TRACE_ALL_COLLIDING: u32 = TRACE_BLOCK_ACTORS | TRACE_MOVERS | TRACE_LEVEL | TRACE_OTHERS;

/// `FCheckResult Hits[64]` capacity in `MultiLineCheck`.
const MAX_LINE_HITS: usize = 64;

/// C++ services (`Hp2LevActServices` in `UnLevAct.cpp`), same field order.
/// Actor and level handles are `void*`; the C++ thunks do the owner calls.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LevActServices {
    /// `new(mem, count) FCheckResult`, returned uninitialized.
    pub alloc_check_results:
        unsafe extern "C" fn(mem: *mut c_void, count: i32) -> *mut FCheckResult,
    /// `Level->GetLevel()->Model->PointCheck(*hit, NULL, loc, ext, 0)`.
    pub model_point_check: unsafe extern "C" fn(
        level: *mut c_void,
        hit: *mut FCheckResult,
        location: *const FVector,
        extent: *const FVector,
    ) -> i32,
    /// `Level->GetLevel()->Model->LineCheck(*hit, NULL, end, start, ext, flags)`.
    pub model_line_check: unsafe extern "C" fn(
        level: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        flags: u32,
    ) -> i32,
    /// `Hash->ActorPointCheck(*mem, loc, ext, flags)`.
    pub hash_actor_point_check: unsafe extern "C" fn(
        hash: *mut c_void,
        mem: *mut c_void,
        location: *const FVector,
        extent: *const FVector,
        flags: u32,
    ) -> *mut FCheckResult,
    /// `Hash->ActorLineCheck(*mem, end, start, ext, flags)`.
    pub hash_actor_line_check: unsafe extern "C" fn(
        hash: *mut c_void,
        mem: *mut c_void,
        end: *const FVector,
        start: *const FVector,
        extent: *const FVector,
        flags: u32,
    ) -> *mut FCheckResult,
    /// `Actor->IsA(kind class)`.
    pub actor_is_a: unsafe extern "C" fn(actor: *const c_void, kind: i32) -> i32,
    /// `Source->IsOwnedBy(Other)`.
    pub actor_is_owned_by: unsafe extern "C" fn(actor: *const c_void, other: *const c_void) -> i32,
    /// `Source->IsBlockedBy(Other)`.
    pub actor_is_blocked_by:
        unsafe extern "C" fn(actor: *const c_void, other: *const c_void) -> i32,
}

/// Services plus the `ULevel` state the checks read: `GMem` and the actor hash
/// (`hash` may be null, as in C++ `if (... && Hash)`).
#[derive(Clone, Copy)]
pub struct LevelCtx<'a> {
    pub svc: &'a LevActServices,
    pub gmem: *mut c_void,
    pub hash: *mut c_void,
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
fn check_result(time: f32) -> FCheckResult {
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

/// `::Min(1.f, x)`: `(A<=B) ? A : B` with `A = 1.f`.
#[inline]
fn min_one(x: f32) -> f32 {
    let one = 1.0f32;
    if one <= x { one } else { x }
}

/// `CompareHits` (`UnPrim.h`): orders hits by `Time` only.
///
/// # Safety
/// Called by `qsort` with two valid `FCheckResult` pointers.
unsafe extern "C" fn compare_hits(a: *const c_void, b: *const c_void) -> c_int {
    unsafe {
        let ta = (*a.cast::<FCheckResult>()).time;
        let tb = (*b.cast::<FCheckResult>()).time;
        if ta < tb {
            -1
        } else if ta > tb {
            1
        } else {
            0
        }
    }
}

unsafe extern "C" {
    /// libc `qsort`, the same routine `appQsort` wraps.
    fn qsort(
        base: *mut c_void,
        nel: usize,
        width: usize,
        compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> c_int>,
    );
}

/// `ULevel::MultiPointCheck(Mem, Location, Extent, ExtraNodeFlags, Level, bActors)`.
/// The level hit is allocated from `GMem` even when `mem` differs, as in C++
/// (`new(GMem)`); that is kept on purpose.
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
) -> *mut FCheckResult {
    unsafe {
        let svc = ctx.svc;
        let mut result: *mut FCheckResult = ptr::null_mut();
        if check_actors && !ctx.hash.is_null() {
            result =
                (svc.hash_actor_point_check)(ctx.hash, mem, &location, &extent, extra_node_flags);
        }
        if !level.is_null() {
            let mut test_hit = check_result(1.0);
            if (svc.model_point_check)(level, &mut test_hit, &location, &extent) == 0 {
                test_hit.next = result.cast();
                result = (svc.alloc_check_results)(ctx.gmem, 1);
                ptr::write(result, test_hit);
                (*result).actor = level;
            }
        }
        result
    }
}

/// `ULevel::SinglePointCheck(Hit, Location, Extent, ExtraNodeFlags, Level, bActors)`
/// body. The C++ shell owns the `FMemMark` around the call. Returns `true` when
/// there is no hit (`return 1`).
///
/// # Safety
/// Same requirements as [`multi_point_check`]; `hit` must be valid.
pub unsafe fn single_point_check(
    ctx: LevelCtx,
    hit: &mut FCheckResult,
    level: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_node_flags: u32,
    check_actors: bool,
) -> bool {
    unsafe {
        let first = multi_point_check(
            ctx,
            ctx.gmem,
            level,
            location,
            extent,
            extra_node_flags,
            check_actors,
        );
        if first.is_null() {
            return true;
        }
        *hit = *first;
        let mut cur = next_of(first);
        while !cur.is_null() {
            if size_squared(sub((*cur).location, location))
                < size_squared(sub(hit.location, location))
            {
                *hit = *cur;
            }
            cur = next_of(cur);
        }
        false
    }
}

/// `ULevel::MultiLineCheck(Mem, End, Start, Extent, bCheckActors, LevelInfo,
/// ExtraNodeFlags)`. The level hit goes in `hits[0]`, hash links fill the rest
/// (cap 64), mover-vs-world ties are adjusted, then the list is qsorted by
/// `Time` and copied into `mem` with `Next` chained.
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
) -> *mut FCheckResult {
    unsafe {
        let LineQuery {
            mut end,
            start,
            extent,
        } = seg;
        let svc = ctx.svc;
        let mut hits = [check_result(1.0); MAX_LINE_HITS];
        let mut num_hits = 0usize;
        let mut dilation = 1.0f32;
        let mut only_check_movers = false;
        let mut hit_world = false;

        if !level.is_null()
            && (svc.model_line_check)(level, &mut hits[0], &end, &start, &extent, extra_node_flags)
                == 0
        {
            hit_world = true;
            hits[0].actor = level;
            let dist = size(sub(hits[0].location, start));
            dilation = min_one(hits[0].time * (dist + 5.0) / (dist + 0.0001));
            end = add(start, scale(sub(end, start), dilation));
            if hits[0].time < 0.01 && dist < 30.0 {
                only_check_movers = true;
            }
            num_hits = 1;
        }

        if (check_actors || !level.is_null()) && !ctx.hash.is_null() {
            let mut link =
                (svc.hash_actor_line_check)(ctx.hash, mem, &end, &start, &extent, extra_node_flags);
            while !link.is_null() && num_hits < MAX_LINE_HITS {
                let is_mover = (svc.actor_is_a)((*link).actor, ACTOR_KIND_MOVER) != 0;
                // C++: `if (!bCheckActors && !IsA(Mover)) continue;`
                if (check_actors || is_mover) && (!only_check_movers || is_mover) {
                    if hit_world
                        && is_mover
                        && (*link).normal == hits[0].normal
                        && size_squared(sub((*link).location, hits[0].location)) < 4.0
                    {
                        let trace_dir = sub(end, start);
                        let trace_dist = size(trace_dir);
                        let trace_dir = div(trace_dir, trace_dist);
                        (*link).location = sub(hits[0].location, scale(trace_dir, 2.0));
                        let t = size(sub((*link).location, start));
                        (*link).time = t / trace_dist;
                    }
                    (*link).time *= dilation;
                    hits[num_hits] = *link;
                    num_hits += 1;
                }
                link = next_of(link);
            }
        }

        if num_hits == 0 {
            return ptr::null_mut();
        }
        qsort(
            hits.as_mut_ptr().cast(),
            num_hits,
            size_of::<FCheckResult>(),
            Some(compare_hits),
        );
        let result = (svc.alloc_check_results)(mem, num_hits as i32);
        for (i, &hit) in hits[..num_hits].iter().enumerate() {
            let mut node = hit;
            node.next = if i + 1 < num_hits {
                result.add(i + 1).cast()
            } else {
                ptr::null_mut()
            };
            ptr::write(result.add(i), node);
        }
        result
    }
}

/// `ULevel::SingleLineCheck(Hit, SourceActor, End, Start, TraceFlags, Extent,
/// ExtraNodeFlags)` body. `level_info` is `GetLevelInfo()`; it is used only when
/// `TRACE_Level` is set. The C++ shell owns the `FMemMark`. Returns `true` when
/// nothing blocks (`return Check==NULL`).
///
/// # Safety
/// Same requirements as [`multi_point_check`]; `source` may be null.
pub unsafe fn single_line_check(
    ctx: LevelCtx,
    hit: &mut FCheckResult,
    source: *mut c_void,
    level_info: *mut c_void,
    seg: LineQuery,
    trace_flags: u32,
    extra_node_flags: u8,
) -> bool {
    unsafe {
        let svc = ctx.svc;
        let level = if trace_flags & TRACE_LEVEL != 0 {
            level_info
        } else {
            ptr::null_mut()
        };
        let first = multi_line_check(
            ctx,
            ctx.gmem,
            level,
            seg,
            trace_flags & TRACE_ALL_COLLIDING != 0,
            u32::from(extra_node_flags),
        );
        let mut check = first;
        while !check.is_null() {
            let actor = (*check).actor;
            if source.is_null() || (svc.actor_is_owned_by)(source, actor) == 0 {
                if (svc.actor_is_a)(actor, ACTOR_KIND_LEVEL_INFO) != 0 {
                    if trace_flags & TRACE_LEVEL != 0 {
                        break;
                    }
                } else if (svc.actor_is_a)(actor, ACTOR_KIND_MOVER) != 0 {
                    if trace_flags & TRACE_MOVERS != 0 {
                        break;
                    }
                } else if (svc.actor_is_a)(actor, ACTOR_KIND_ZONE_INFO) != 0 {
                    if trace_flags & TRACE_ZONE_CHANGES != 0 {
                        break;
                    }
                } else if !source.is_null() && (svc.actor_is_blocked_by)(source, actor) != 0 {
                    if trace_flags & TRACE_BLOCK_ACTORS != 0 {
                        break;
                    }
                } else if trace_flags & TRACE_OTHERS != 0 {
                    break;
                }
            }
            check = next_of(check);
        }
        if !check.is_null() {
            *hit = *check;
        } else {
            hit.time = 1.0;
            hit.actor = ptr::null_mut();
        }
        check.is_null()
    }
}
