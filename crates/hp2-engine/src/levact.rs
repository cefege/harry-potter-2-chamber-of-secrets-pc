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
use hp2_abi::engine::generated::{
    AActor, APawn, FCheckResult, FPointRegion, FRotator, TArrayRaw, ULevel,
};

use crate::math::app_sqrt;
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
    pub mem_mark_save: unsafe extern "C" fn(mem: *mut c_void, buffer: *mut c_void),
    pub mem_mark_pop: unsafe extern "C" fn(buffer: *mut c_void),
    pub app_cycles: unsafe extern "C" fn() -> u32,
    pub is_editor: unsafe extern "C" fn() -> i32,
    pub bump_observer: unsafe extern "C" fn(recipient: *mut c_void, other: *mut c_void),
    pub sin_tab: unsafe extern "C" fn(angle: i32) -> f32,
    pub cos_tab: unsafe extern "C" fn(angle: i32) -> f32,
    pub model_point_region: unsafe extern "C" fn(
        model: *mut c_void,
        zone: *mut c_void,
        location: *const FVector,
        out: *mut FPointRegion,
    ),
    pub hash_add_actor: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void),
    pub hash_remove_actor: unsafe extern "C" fn(hash: *mut c_void, actor: *mut c_void),
    pub hash_actor_encroachment_check: unsafe extern "C" fn(
        hash: *mut c_void,
        mem: *mut c_void,
        actor: *mut c_void,
        location: *const FVector,
        rotation: *const FRotator,
        flags: u32,
    ) -> *mut FCheckResult,
    pub tracker_update: unsafe extern "C" fn(tracker: *mut c_void, actor: *mut c_void),
    pub actor_begin_touch: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void),
    pub actor_end_touch:
        unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void, no_notify: i32),
    pub actor_is_overlapping: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub actor_collision_box:
        unsafe extern "C" fn(actor: *mut c_void, min: *mut FVector, max: *mut FVector),
    pub actor_cylinder_extent: unsafe extern "C" fn(actor: *mut c_void, out: *mut FVector),
    pub actor_set_base: unsafe extern "C" fn(actor: *mut c_void, base: *mut c_void),
    pub actor_move_smooth: unsafe extern "C" fn(actor: *mut c_void, delta: *const FVector) -> i32,
    pub event_bump: unsafe extern "C" fn(recipient: *mut c_void, other: *mut c_void),
    pub event_encroaching_on: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void) -> i32,
    pub event_encroached_by: unsafe extern "C" fn(actor: *mut c_void, other: *mut c_void),
    pub event_zone_change: unsafe extern "C" fn(actor: *mut c_void, zone: *mut c_void),
    pub zone_event_actor_leaving: unsafe extern "C" fn(zone: *mut c_void, actor: *mut c_void),
    pub zone_event_actor_entered: unsafe extern "C" fn(zone: *mut c_void, actor: *mut c_void),
    pub pawn_event_foot_zone_change: unsafe extern "C" fn(pawn: *mut c_void, zone: *mut c_void),
    pub pawn_event_head_zone_change: unsafe extern "C" fn(pawn: *mut c_void, zone: *mut c_void),
    pub pri_set_player_zone: unsafe extern "C" fn(pri: *mut c_void, zone: *mut c_void),
    pub level_net_mode: unsafe extern "C" fn(level_info: *mut c_void) -> u8,
}

/// Services plus the `ULevel` state the checks read: `GMem` and the actor hash
/// (`hash` may be null, as in C++ `if (... && Hash)`).
#[derive(Clone, Copy)]
pub struct LevelCtx<'a> {
    pub svc: &'a LevActServices,
    pub gmem: *mut c_void,
    pub hash: *mut c_void,
    pub ulevel: *mut ULevel,
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
/// Returns `ULevel::Actors(0)`, including the original unchecked empty-array case.
///
/// # Safety
/// `ctx.ulevel` must point to a live `ULevel` whose actor array follows its ABI layout.
unsafe fn level_info(ctx: LevelCtx<'_>) -> *mut c_void {
    unsafe {
        let array = ptr::addr_of!((*ctx.ulevel).actors)
            .cast::<TArrayRaw>()
            .read_unaligned();
        array.data.cast::<*mut c_void>().read_unaligned()
    }
}

/// Saves and pops an engine `FMemMark` around an internal single-check call.
///
/// # Safety
/// `ctx.gmem` and the matching mark services must be live.
unsafe fn save_mark(ctx: LevelCtx<'_>) -> [u64; 3] {
    let mut mark = [0u64; 3];
    unsafe { (ctx.svc.mem_mark_save)(ctx.gmem, mark.as_mut_ptr().cast()) };
    mark
}

/// `ULevel::AdjustSpot`.
///
/// # Safety
/// Context and service pointers must be live.
pub unsafe fn adjust_spot(
    ctx: LevelCtx<'_>,
    adjusted: &mut FVector,
    trace_dest: FVector,
    trace_len: f32,
    hit: &mut FCheckResult,
) {
    unsafe {
        let mark = save_mark(ctx);
        let _ = single_line_check(
            ctx,
            hit,
            ptr::null_mut(),
            level_info(ctx),
            LineQuery {
                end: trace_dest,
                start: *adjusted,
                extent: zero(),
            },
            TRACE_LEVEL | TRACE_MOVERS,
            0,
        );
        (ctx.svc.mem_mark_pop)(mark.as_ptr().cast_mut().cast());
        if hit.time < 1.0 {
            // `Hit.Normal * (1.05f - Hit.Time) * TraceLen`: two vector scalings, unfused.
            *adjusted = add(
                *adjusted,
                scale(scale(hit.normal, 1.05 - hit.time), trace_len),
            );
        }
    }
}

/// `ULevel::FindSpot`.
///
/// # Safety
/// Context and service pointers must be live.
pub unsafe fn find_spot(
    ctx: LevelCtx<'_>,
    extent: FVector,
    location: &mut FVector,
    check_actors: bool,
    assume_fit: bool,
) -> bool {
    unsafe {
        let info = level_info(ctx);
        let mut hit = check_result(1.0);
        let point_check = |ctx: LevelCtx<'_>, hit: &mut FCheckResult, at: FVector| {
            let mark = save_mark(ctx);
            let clear = single_point_check(ctx, hit, info, at, extent, 0, check_actors);
            (ctx.svc.mem_mark_pop)(mark.as_ptr().cast_mut().cast());
            clear
        };
        if extent.x == 0.0 && extent.y == 0.0 && extent.z == 0.0 {
            return point_check(ctx, &mut hit, *location);
        }
        if assume_fit && point_check(ctx, &mut hit, *location) {
            return true;
        }
        let original = *location;
        let mut adjusted = original;
        let trace_len = size(extent) + 2.0;
        for i in [-1.0_f32, 1.0] {
            let dest_x = add(
                adjusted,
                FVector {
                    x: i * extent.x,
                    y: 0.0,
                    z: 0.0,
                },
            );
            adjust_spot(ctx, &mut adjusted, dest_x, extent.x, &mut hit);
            let dest_y = add(
                adjusted,
                FVector {
                    x: 0.0,
                    y: i * extent.y,
                    z: 0.0,
                },
            );
            adjust_spot(ctx, &mut adjusted, dest_y, extent.y, &mut hit);
            let dest_z = add(
                adjusted,
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: i * extent.z,
                },
            );
            adjust_spot(ctx, &mut adjusted, dest_z, extent.z, &mut hit);
        }
        if point_check(ctx, &mut hit, adjusted) {
            *location = adjusted;
            return true;
        }
        for i in [-1.0_f32, 1.0] {
            for j in [-1.0_f32, 1.0] {
                for k in [-1.0_f32, 1.0] {
                    let destination = add(
                        adjusted,
                        FVector {
                            x: i * extent.x,
                            y: j * extent.y,
                            z: k * extent.z,
                        },
                    );
                    adjust_spot(ctx, &mut adjusted, destination, trace_len, &mut hit);
                }
            }
        }
        let displacement = sub(adjusted, original);
        if f64::from(size_squared(displacement)) > 1.5_f64 * f64::from(size_squared(extent)) {
            return false;
        }
        if point_check(ctx, &mut hit, adjusted) {
            *location = adjusted;
            return true;
        }
        false
    }
}

/// `ULevel::SetActorZone`.
///
/// # Safety
/// `actor`, `ctx.ulevel`, and every non-null engine service pointer must be live.
pub unsafe fn set_actor_zone(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    test: bool,
    force_refresh: bool,
) {
    unsafe {
        let a = actor.cast::<AActor>();
        if (*a).b_delete_me() {
            return;
        }
        let info = level_info(ctx);
        let mut fallback: FPointRegion = std::mem::zeroed();
        fallback.zone = info;
        fallback.i_leaf = -1;
        if actor == info {
            ptr::addr_of_mut!((*a).region).write_unaligned(fallback);
            return;
        }
        let pawn = if (ctx.svc.actor_is_a)(actor, ACTOR_KIND_PAWN) != 0 {
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
        let actors = ptr::addr_of!((*ctx.ulevel).actors)
            .cast::<TArrayRaw>()
            .read_unaligned();
        let zone = if actors.num != 0 { info } else { actor };
        let model = ptr::addr_of!((*ctx.ulevel).model).read_unaligned();
        let location = ptr::addr_of!((*a).location).read_unaligned();
        let mut new_region = fallback;
        (ctx.svc.model_point_region)(model, zone, &location, &mut new_region);
        let old_region = ptr::addr_of!((*a).region).read_unaligned();
        if new_region.zone != old_region.zone {
            if !test {
                (ctx.svc.zone_event_actor_leaving)(old_region.zone, actor);
                (ctx.svc.event_zone_change)(actor, new_region.zone);
            }
            ptr::addr_of_mut!((*a).region).write_unaligned(new_region);
            if !test {
                (ctx.svc.zone_event_actor_entered)(new_region.zone, actor);
            }
        } else {
            ptr::addr_of_mut!((*a).region).write_unaligned(new_region);
        }
        if !pawn.is_null() {
            let height = ptr::addr_of!((*a).collision_height).read_unaligned();
            let foot = sub(
                location,
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: height,
                },
            );
            let mut new_foot = fallback;
            (ctx.svc.model_point_region)(model, info, &foot, &mut new_foot);
            let old_foot = ptr::addr_of!((*pawn).foot_region).read_unaligned();
            if new_foot.zone != old_foot.zone && !test {
                (ctx.svc.pawn_event_foot_zone_change)(pawn.cast(), new_foot.zone);
            }
            ptr::addr_of_mut!((*pawn).foot_region).write_unaligned(new_foot);
            let eye_height = ptr::addr_of!((*pawn).eye_height).read_unaligned();
            let head = add(
                location,
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: eye_height,
                },
            );
            let mut new_head = fallback;
            (ctx.svc.model_point_region)(model, info, &head, &mut new_head);
            let old_head = ptr::addr_of!((*pawn).head_region).read_unaligned();
            if new_head.zone != old_head.zone && !test {
                (ctx.svc.pawn_event_head_zone_change)(pawn.cast(), new_head.zone);
            }
            ptr::addr_of_mut!((*pawn).head_region).write_unaligned(new_head);
            let pri = ptr::addr_of!((*pawn).player_replication_info).read_unaligned();
            if !pri.is_null() && (ctx.svc.level_net_mode)(info) != 3 {
                (ctx.svc.pri_set_player_zone)(pri, new_region.zone);
            }
        }
    }
}

/// `AActor::IsMovingBrush()` as used by the level movement code.
///
/// # Safety
/// `actor` and the actor-kind service must be live.
unsafe fn is_moving_brush(ctx: LevelCtx<'_>, actor: *mut AActor) -> bool {
    unsafe {
        !ptr::addr_of!((*actor).brush).read_unaligned().is_null()
            && (ctx.svc.actor_is_a)(actor.cast(), ACTOR_KIND_BRUSH) != 0
            && !(*actor).b_static()
    }
}

/// `ULevel::CheckEncroachment`.
///
/// # Safety
/// Actor, context, and engine-owned memory must be live. The original C++ body
/// calls the recheck hash method without checking `Hash`; this preserves that UB.
pub unsafe fn check_encroachment(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    test_location: FVector,
    test_rotation: FRotator,
    touch_notify: bool,
) -> bool {
    unsafe {
        let a = actor.cast::<AActor>();
        if !(*a).b_collide_actors()
            && !(*a).b_block_actors()
            && !(*a).b_block_players()
            && !is_moving_brush(ctx, a)
        {
            return false;
        }
        let mark = save_mark(ctx);
        let mut first = ptr::null_mut();
        if !ctx.hash.is_null() {
            first = (ctx.svc.hash_actor_encroachment_check)(
                ctx.hash,
                ctx.gmem,
                actor,
                &test_location,
                &test_rotation,
                0,
            );
        }
        let level_info = level_info(ctx);
        let mut test = first;
        while !test.is_null() {
            let other = (*test).actor;
            let mut no_process = false;
            if other != actor
                && other != level_info
                && (ctx.svc.actor_is_blocked_by)(actor, other) != 0
            {
                let other_actor = other.cast::<AActor>();
                if is_moving_brush(ctx, a) && !is_moving_brush(ctx, other_actor) {
                    let actor_location = ptr::addr_of!((*a).location).read_unaligned();
                    let other_location = ptr::addr_of!((*other_actor).location).read_unaligned();
                    let actor_rotation = ptr::addr_of!((*a).rotation).read_unaligned();
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
                    let _ = (ctx.svc.actor_move_smooth)(other, &move_dir);
                    // Deliberately no Hash-null guard: the C++ recheck is unconditional here.
                    let recheck = (ctx.svc.hash_actor_encroachment_check)(
                        ctx.hash,
                        ctx.gmem,
                        actor,
                        &test_location,
                        &test_rotation,
                        0,
                    );
                    no_process = true;
                    let mut re = recheck;
                    while !re.is_null() {
                        if (*re).actor == other {
                            no_process = false;
                            break;
                        }
                        re = next_of(re);
                    }
                    if !no_process {
                        let real_location = ptr::addr_of!((*a).location).read_unaligned();
                        ptr::addr_of_mut!((*a).location).write_unaligned(test_location);
                        let reverse = scale(move_dir, -1.0);
                        let _ = (ctx.svc.actor_move_smooth)(other, &reverse);
                        ptr::addr_of_mut!((*a).location).write_unaligned(real_location);
                    }
                }
                if !no_process && (ctx.svc.event_encroaching_on)(actor, other) != 0 {
                    (ctx.svc.mem_mark_pop)(mark.as_ptr().cast_mut().cast());
                    return true;
                }
            }
            test = next_of(test);
        }
        if touch_notify {
            for i in 0..4 {
                let touching = ptr::addr_of!((*a).touching)
                    .cast::<*mut c_void>()
                    .add(i)
                    .read_unaligned();
                if !touching.is_null() && (ctx.svc.actor_is_overlapping)(actor, touching) == 0 {
                    (ctx.svc.actor_end_touch)(actor, touching, 0);
                }
            }
        }
        test = first;
        while !test.is_null() {
            let other = (*test).actor;
            if other != actor && other != level_info {
                if (ctx.svc.actor_is_blocked_by)(actor, other) != 0 {
                    (ctx.svc.event_encroached_by)(other, actor);
                } else if touch_notify {
                    (ctx.svc.actor_begin_touch)(actor, other);
                }
            }
            test = next_of(test);
        }
        (ctx.svc.mem_mark_pop)(mark.as_ptr().cast_mut().cast());
        false
    }
}

/// `ULevel::Actors(index)`.
///
/// # Safety
/// The live level's actor array must contain `index`.
unsafe fn actor_at(ctx: LevelCtx<'_>, index: usize) -> *mut c_void {
    unsafe {
        let array = ptr::addr_of!((*ctx.ulevel).actors)
            .cast::<TArrayRaw>()
            .read_unaligned();
        array.data.cast::<*mut c_void>().add(index).read_unaligned()
    }
}

/// `Actors.Num()`, read fresh at every call like the C++ loop conditions do.
///
/// # Safety
/// `ctx.ulevel` must be live.
unsafe fn actor_count(ctx: LevelCtx<'_>) -> usize {
    unsafe {
        ptr::addr_of!((*ctx.ulevel).actors)
            .cast::<TArrayRaw>()
            .read_unaligned()
            .num as usize
    }
}

/// `ULevel::FarMoveActor`.
///
/// # Safety
/// Actor, level, and engine services must be live.
pub unsafe fn far_move_actor(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    destination: FVector,
    test: bool,
    no_check: bool,
) -> bool {
    unsafe {
        let a = actor.cast::<AActor>();
        if ((*a).b_static() || !(*a).b_movable()) && (ctx.svc.is_editor)() == 0 {
            return false;
        }
        if (*a).b_collide_actors() && !ctx.hash.is_null() {
            (ctx.svc.hash_remove_actor)(ctx.hash, actor);
        }
        let info = level_info(ctx);
        let mut new_location = destination;
        let mut result = true;
        if !no_check
            && ((*a).b_collide_world()
                || ((*a).b_collide_when_placing() && (ctx.svc.level_net_mode)(info) != 3))
        {
            let mut extent = zero();
            (ctx.svc.actor_cylinder_extent)(actor, &mut extent);
            result = find_spot(ctx, extent, &mut new_location, false, false);
        }
        if result && !test && !no_check {
            let rotation = ptr::addr_of!((*a).rotation).read_unaligned();
            result = !check_encroachment(ctx, actor, new_location, rotation, true);
        }
        if result {
            if !test && (ctx.svc.level_net_mode)(info) != 3 {
                let standing = (*a).standing_count;
                if standing > 0 {
                    let mut i = 0;
                    while i < actor_count(ctx) {
                        let other = actor_at(ctx, i);
                        if !other.is_null()
                            && ptr::addr_of!((*other.cast::<AActor>()).base).read_unaligned()
                                == actor
                        {
                            (ctx.svc.actor_set_base)(other, ptr::null_mut());
                        }
                        i += 1;
                    }
                }
                (*a).set_b_just_teleported(true);
            }
            ptr::addr_of_mut!((*a).location).write_unaligned(new_location);
            ptr::addr_of_mut!((*a).old_location).write_unaligned(new_location);
        }
        if (*a).b_collide_actors() && !ctx.hash.is_null() {
            (ctx.svc.hash_add_actor)(ctx.hash, actor);
        }
        let tracker = ptr::addr_of!((*ctx.ulevel).brush_tracker).read_unaligned();
        (ctx.svc.tracker_update)(tracker, actor);
        if result {
            set_actor_zone(ctx, actor, test, false);
        }
        result
    }
}

fn rot_eq(a: FRotator, b: FRotator) -> bool {
    a.pitch == b.pitch && a.yaw == b.yaw && a.roll == b.roll
}

fn nearly_zero(v: FVector) -> bool {
    v.x.abs() < 1.0e-4 && v.y.abs() < 1.0e-4 && v.z.abs() < 1.0e-4
}

/// `ULevel::MoveActor`.
///
/// # Safety
/// `actor`, context, actor arrays, and all service pointers used by the selected
/// branches must be live. The early failure paths intentionally retain the
/// original unpopped mark behavior.
#[allow(clippy::too_many_arguments)]
pub unsafe fn move_actor(
    ctx: LevelCtx<'_>,
    actor: *mut c_void,
    delta: FVector,
    new_rotation: FRotator,
    hit: &mut FCheckResult,
    test: bool,
    ignore_pawns: bool,
    ignore_bases: bool,
    no_fail: bool,
) -> bool {
    use hp2_abi::engine::generated::AMover;
    unsafe {
        let a = actor.cast::<AActor>();
        if ((*a).b_static() || !(*a).b_movable()) && (ctx.svc.is_editor)() == 0 {
            return false;
        }
        let rotation = ptr::addr_of!((*a).rotation).read_unaligned();
        let moving_brush = is_moving_brush(ctx, a);
        if nearly_zero(delta) {
            if rot_eq(new_rotation, rotation) {
                return true;
            }
            if (*a).standing_count == 0 && !moving_brush {
                ptr::addr_of_mut!((*a).rotation).write_unaligned(new_rotation);
                return true;
            }
        }
        *hit = check_result(1.0);
        (*ctx.ulevel).num_moves = (*ctx.ulevel).num_moves.wrapping_add(1);
        let cycles = (ctx.svc.app_cycles)();
        (*ctx.ulevel).move_cycles = (*ctx.ulevel).move_cycles.wrapping_sub(cycles as i32);
        let mark = save_mark(ctx);
        let delta_size = if nearly_zero(delta) { 0.0 } else { size(delta) };
        let delta_dir = if nearly_zero(delta) {
            delta
        } else {
            div(delta, delta_size)
        };
        let test_adjust = if moving_brush { 0.0 } else { 2.0 };
        let test_delta = add(delta, scale(delta_dir, test_adjust));
        let mut maybe_touched = false;
        let mut first_hit = ptr::null_mut();
        if ((*a).b_collide_actors() || (*a).b_collide_world())
            && (delta.x != 0.0 || delta.y != 0.0 || delta.z != 0.0)
        {
            let mut min = zero();
            let mut max = zero();
            (ctx.svc.actor_collision_box)(actor, &mut min, &mut max);
            let trace_location = scale(add(min, max), 0.5);
            let mut extent = scale(sub(max, min), 0.5);
            if moving_brush {
                extent = sub(
                    extent,
                    FVector {
                        x: 0.51,
                        y: 0.51,
                        z: 0.51,
                    },
                );
            }
            let is_mover = (ctx.svc.actor_is_a)(actor, ACTOR_KIND_MOVER) != 0;
            let encroach_type = if is_mover {
                ptr::addr_of!((*actor.cast::<AMover>()).mover_encroach_type).read_unaligned()
            } else {
                0
            };
            if do_multi_line_check(is_mover, encroach_type) {
                let start = trace_location;
                first_hit = multi_line_check(
                    ctx,
                    ctx.gmem,
                    if (*a).b_collide_world() {
                        level_info(ctx)
                    } else {
                        ptr::null_mut()
                    },
                    LineQuery {
                        end: add(trace_location, test_delta),
                        start,
                        extent,
                    },
                    (*a).b_collide_actors(),
                    0,
                );
            }
        }
        if (*a).b_collide_world() || (*a).b_block_actors() || (*a).b_block_players() {
            let mut check = first_hit;
            while !check.is_null() {
                let other = (*check).actor;
                let other_actor = other.cast::<AActor>();
                let pawn_or_decoration = (ctx.svc.actor_is_a)(other, ACTOR_KIND_PAWN) != 0
                    || (ctx.svc.actor_is_a)(other, ACTOR_KIND_DECORATION) != 0;
                if (!ignore_pawns || (*other_actor).b_static() || !pawn_or_decoration)
                    && (!ignore_bases || !is_based_on(actor, other))
                    && !is_based_on(other, actor)
                {
                    maybe_touched = true;
                    if (ctx.svc.actor_is_blocked_by)(actor, other) != 0 {
                        *hit = *check;
                        break;
                    }
                }
                check = next_of(check);
            }
        }
        let mut final_delta = delta;
        if hit.time < 1.0 && !no_fail {
            let final_delta_size = (delta_size + test_adjust) * hit.time;
            hit.time = (final_delta_size - test_adjust) / delta_size;
            if hit.time <= 1.0e-4 {
                hit.time = 0.0;
                return false;
            }
            final_delta = scale(delta, hit.time);
        }
        if (*a).standing_count != 0 && !test {
            let mut i = 0;
            while i < actor_count(ctx) {
                let other = actor_at(ctx, i);
                i += 1;
                if other.is_null()
                    || ptr::addr_of!((*other.cast::<AActor>()).base).read_unaligned() != actor
                {
                    continue;
                }
                let other_actor = other.cast::<AActor>();
                let mut rot_motion = zero();
                // The C++ re-reads `Actor->Rotation` on every pass.
                let rotation = ptr::addr_of!((*a).rotation).read_unaligned();
                let yaw_delta = new_rotation.yaw.wrapping_sub(rotation.yaw);
                let delta_rot = FRotator {
                    pitch: 0,
                    yaw: yaw_delta,
                    roll: 0,
                };
                if !rot_eq(new_rotation, rotation) {
                    let reduced = FRotator {
                        pitch: 0,
                        yaw: (new_rotation.yaw & 0xfffc).wrapping_sub(rotation.yaw & 0xfffc),
                        roll: 0,
                    };
                    let pointer = sub(
                        ptr::addr_of!((*a).location).read_unaligned(),
                        ptr::addr_of!((*other_actor).location).read_unaligned(),
                    );
                    let sin = |angle| (ctx.svc.sin_tab)(angle);
                    let cos = |angle| (ctx.svc.cos_tab)(angle);
                    let trig = crate::coords::Trig {
                        sin: &sin,
                        cos: &cos,
                    };
                    let mut coords = crate::coords::unit_coords();
                    crate::coords::mul_rot(&mut coords, reduced, &trig);
                    rot_motion = sub(pointer, crate::prim::transform_vector(pointer, &coords));
                }
                let other_rotation = ptr::addr_of!((*other_actor).rotation).read_unaligned();
                let mut local_hit = check_result(1.0);
                let _ = move_actor(
                    ctx,
                    other,
                    add(final_delta, rot_motion),
                    FRotator {
                        pitch: other_rotation.pitch,
                        yaw: other_rotation.yaw.wrapping_add(delta_rot.yaw),
                        roll: other_rotation.roll,
                    },
                    &mut local_hit,
                    false,
                    false,
                    true,
                    false,
                );
                if (ctx.svc.actor_is_a)(other, ACTOR_KIND_PAWN) != 0 {
                    let pawn = other.cast::<APawn>();
                    let view = ptr::addr_of!((*pawn).view_rotation).read_unaligned();
                    ptr::addr_of_mut!((*pawn).view_rotation).write_unaligned(FRotator {
                        pitch: view.pitch,
                        yaw: view.yaw.wrapping_add(delta_rot.yaw),
                        roll: view.roll,
                    });
                }
            }
        }
        if (*a).b_collide_actors() && !ctx.hash.is_null() {
            (ctx.svc.hash_remove_actor)(ctx.hash, actor);
        }
        if !test
            && !no_fail
            && (ctx.svc.actor_is_a)(actor, ACTOR_KIND_PAWN) == 0
            && check_encroachment(
                ctx,
                actor,
                add(ptr::addr_of!((*a).location).read_unaligned(), final_delta),
                new_rotation,
                false,
            )
        {
            let now = (ctx.svc.app_cycles)();
            (*ctx.ulevel).move_cycles = (*ctx.ulevel)
                .move_cycles
                .wrapping_add(now.wrapping_sub(34) as i32);
            if (*a).b_collide_actors() && !ctx.hash.is_null() {
                (ctx.svc.hash_add_actor)(ctx.hash, actor);
            }
            return false;
        }
        let location = add(ptr::addr_of!((*a).location).read_unaligned(), final_delta);
        ptr::addr_of_mut!((*a).location).write_unaligned(location);
        ptr::addr_of_mut!((*a).rotation).write_unaligned(new_rotation);
        if (*a).b_collide_actors() && !ctx.hash.is_null() {
            (ctx.svc.hash_add_actor)(ctx.hash, actor);
        }
        let tracker = ptr::addr_of!((*ctx.ulevel).brush_tracker).read_unaligned();
        (ctx.svc.tracker_update)(tracker, actor);
        if !test {
            let hit_actor = hit.actor;
            if !hit_actor.is_null()
                && hit_actor != level_info(ctx)
                && !is_based_on(actor, hit_actor)
            {
                (ctx.svc.bump_observer)(hit_actor, actor);
                (ctx.svc.event_bump)(hit_actor, actor);
                (ctx.svc.bump_observer)(actor, hit_actor);
                (ctx.svc.event_bump)(actor, hit_actor);
            }
            if maybe_touched || !(*a).b_block_actors() || !(*a).b_block_players() {
                let mut check = first_hit;
                while !check.is_null() && (*check).time < hit.time {
                    let other = (*check).actor;
                    if !is_based_on(other, actor)
                        && (!ignore_bases || !is_based_on(actor, other))
                        && (ctx.svc.actor_is_blocked_by)(actor, other) == 0
                    {
                        (ctx.svc.actor_begin_touch)(actor, other);
                    }
                    check = next_of(check);
                }
            }
            for i in 0..4 {
                let touching = ptr::addr_of!((*a).touching)
                    .cast::<*mut c_void>()
                    .add(i)
                    .read_unaligned();
                if !touching.is_null() && (ctx.svc.actor_is_overlapping)(actor, touching) == 0 {
                    (ctx.svc.actor_end_touch)(actor, touching, 0);
                }
            }
        }
        set_actor_zone(ctx, actor, test, false);
        (ctx.svc.mem_mark_pop)(mark.as_ptr().cast_mut().cast());
        let now = (ctx.svc.app_cycles)();
        (*ctx.ulevel).move_cycles = (*ctx.ulevel)
            .move_cycles
            .wrapping_add(now.wrapping_sub(34) as i32);
        hit.time > 0.0
    }
}

/// `AActor::IsBasedOn`: walks `Base` from `actor`, never comparing a null link.
unsafe fn is_based_on(actor: *mut c_void, other: *mut c_void) -> bool {
    unsafe {
        let mut test = actor.cast::<AActor>();
        while !test.is_null() {
            if test.cast::<c_void>() == other {
                return true;
            }
            test = ptr::addr_of!((*test).base)
                .read_unaligned()
                .cast::<AActor>();
        }
        false
    }
}
