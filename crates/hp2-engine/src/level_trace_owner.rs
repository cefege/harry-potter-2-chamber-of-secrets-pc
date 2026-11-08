//! Canonical Rust Level single-check operations shared by Level and Actor policies.

use crate::{actor, collision_owner};
use hp2_abi::engine::generated::{AActor, FCheckResult, ULevel, UModel};
use hp2_abi::FVector;
use std::ffi::c_void;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
const TRACE_BLOCK_ACTORS: u32 = 0x01;
const TRACE_MOVERS: u32 = 0x02;
const TRACE_LEVEL: u32 = 0x04;
const TRACE_ZONE_CHANGES: u32 = 0x08;
const TRACE_OTHERS: u32 = 0x10;

#[derive(Clone, Copy)]
pub(crate) enum LineActorKind {
    LevelInfo,
    Mover,
    ZoneInfo,
}

/// Backend queries used by the shared `SingleLineCheck` source/filter policy.
pub(crate) trait LineFilterOps {
    type Error;
    unsafe fn is_owned_by(&mut self, source: *mut AActor, other: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn is_kind(&mut self, actor: *mut AActor, kind: LineActorKind) -> Result<bool, Self::Error>;
    unsafe fn is_blocked_by(&mut self, source: *mut AActor, other: *mut AActor) -> Result<bool, Self::Error>;
}

/// Native/Core leaves used by shared `ULevel::MultiPointCheck` policy.
pub(crate) trait MultiPointOps {
    type Error;
    unsafe fn actor_point_check(
        &mut self,
        memory: *mut c_void,
        location: FVector,
        extent: FVector,
        extra_flags: u32,
    ) -> Result<*mut FCheckResult, Self::Error>;
    unsafe fn world_point_check(
        &mut self,
        level_info: *mut c_void,
        hit: *mut FCheckResult,
        location: FVector,
        extent: FVector,
        extra_flags: u32,
    ) -> Result<bool, Self::Error>;
    unsafe fn alloc_result(
        &mut self,
        memory: *mut c_void,
        result: FCheckResult,
    ) -> Result<*mut FCheckResult, Self::Error>;
}

/// Runs native actor-first/world-second result assembly for MultiPointCheck.
pub(crate) unsafe fn multi_point_check_with_ops<O: MultiPointOps>(
    ops: &mut O,
    memory: *mut c_void,
    level_info: *mut c_void,
    location: FVector,
    extent: FVector,
    extra_flags: u32,
    check_actors: bool,
) -> Result<*mut FCheckResult, O::Error> {
    let mut result = if check_actors {
        unsafe { ops.actor_point_check(memory, location, extent, extra_flags)? }
    } else {
        std::ptr::null_mut()
    };
    if !level_info.is_null() {
        let mut world_hit: FCheckResult = unsafe { std::mem::zeroed() };
        if !unsafe { ops.world_point_check(level_info, &mut world_hit, location, extent, extra_flags)? } {
            world_hit.actor = level_info;
            world_hit.next = result.cast();
            result = unsafe { ops.alloc_result(memory, world_hit)? };
        }
    }
    Ok(result)
}
#[inline]
fn min_one(value: f32) -> f32 {
    let one = 1.0_f32;
    if one <= value { one } else { value }
}

/// Native/Core leaves used by shared `ULevel::MultiLineCheck` policy.
pub(crate) trait MultiLineOps {
    type Error;
    unsafe fn world_line_check(
        &mut self,
        level_info: *mut c_void,
        hit: *mut FCheckResult,
        end: FVector,
        start: FVector,
        extent: FVector,
        flags: u32,
    ) -> Result<bool, Self::Error>;
    unsafe fn actor_line_check(
        &mut self,
        memory: *mut c_void,
        end: FVector,
        start: FVector,
        extent: FVector,
        flags: u32,
    ) -> Result<*mut FCheckResult, Self::Error>;
    unsafe fn is_mover(&mut self, actor: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn alloc_results(&mut self, memory: *mut c_void, count: usize) -> Result<*mut FCheckResult, Self::Error>;
}

/// Runs the native world/actor merge, mover tie adjustment, and hit ordering.
pub(crate) unsafe fn multi_line_check_with_ops<O: MultiLineOps>(
    ops: &mut O,
    memory: *mut c_void,
    level_info: *mut c_void,
    mut end: FVector,
    start: FVector,
    extent: FVector,
    check_actors: bool,
    extra_flags: u32,
) -> Result<*mut FCheckResult, O::Error> {
    const MAX_LINE_HITS: usize = 64;
    let mut hits = [unsafe { std::mem::zeroed::<FCheckResult>() }; MAX_LINE_HITS];
    let mut num_hits = 0usize;
    let mut dilation = 1.0_f32;
    let mut only_check_movers = false;
    let mut hit_world = false;
    if !level_info.is_null() {
        let mut world_hit = unsafe { std::mem::zeroed::<FCheckResult>() };
        if !unsafe { ops.world_line_check(level_info, &mut world_hit, end, start, extent, extra_flags)? } {
            world_hit.actor = level_info;
            let distance = crate::vec::size(crate::vec::sub(world_hit.location, start));
            dilation = min_one(world_hit.time * (distance + 5.0) / (distance + 0.0001));
            end = crate::vec::add(start, crate::vec::scale(crate::vec::sub(end, start), dilation));
            only_check_movers = world_hit.time < 0.01 && distance < 30.0;
            hit_world = true;
            hits[num_hits] = world_hit;
            num_hits += 1;
        }
    }
    if (check_actors || !level_info.is_null()) && num_hits < MAX_LINE_HITS {
        let mut link = unsafe { ops.actor_line_check(memory, end, start, extent, extra_flags)? };
        while !link.is_null() && num_hits < MAX_LINE_HITS {
            let next = unsafe { (*link).next.cast::<FCheckResult>() };
            let mover = unsafe { ops.is_mover((*link).actor.cast())? };
            if (check_actors || mover) && (!only_check_movers || mover) {
                let mut candidate = unsafe { *link };
                if hit_world && mover && candidate.normal == hits[0].normal
                    && crate::vec::size_squared(crate::vec::sub(candidate.location, hits[0].location)) < 4.0
                {
                    let trace_dir = crate::vec::sub(end, start);
                    let trace_dist = crate::vec::size(trace_dir);
                    let trace_dir = crate::vec::div(trace_dir, trace_dist);
                    candidate.location = crate::vec::sub(hits[0].location, crate::vec::scale(trace_dir, 2.0));
                    candidate.time = crate::vec::size(crate::vec::sub(candidate.location, start)) / trace_dist;
                }
                candidate.time *= dilation;
                hits[num_hits] = candidate;
                num_hits += 1;
            }
            link = next;
        }
    }
    hits[..num_hits].sort_unstable_by(|left, right| {
        if left.time < right.time { std::cmp::Ordering::Less }
        else if left.time > right.time { std::cmp::Ordering::Greater }
        else { std::cmp::Ordering::Equal }
    });
    if num_hits == 0 {
        return Ok(std::ptr::null_mut());
    }
    let result = unsafe { ops.alloc_results(memory, num_hits)? };
    for index in 0..num_hits {
        let mut node = hits[index];
        node.next = if index + 1 < num_hits {
            unsafe { result.add(index + 1).cast() }
        } else {
            std::ptr::null_mut()
        };
        unsafe { result.add(index).write(node); }
    }
    Ok(result)
}

/// Applies native source ownership, actor kind, and blocking filters in hit-list order.
pub(crate) unsafe fn filter_line_hits_with_ops<O: LineFilterOps>(
    ops: &mut O,
    hit: *mut FCheckResult,
    source: *mut AActor,
    first: *mut FCheckResult,
    trace_flags: u32,
) -> Result<bool, O::Error> {
    let mut check = first;
    while !check.is_null() {
        let actor = unsafe { (*check).actor.cast::<AActor>() };
        if source.is_null() || !unsafe { ops.is_owned_by(source, actor)? } {
            if unsafe { ops.is_kind(actor, LineActorKind::LevelInfo)? } {
                if trace_flags & TRACE_LEVEL != 0 { break; }
            } else if unsafe { ops.is_kind(actor, LineActorKind::Mover)? } {
                if trace_flags & TRACE_MOVERS != 0 { break; }
            } else if unsafe { ops.is_kind(actor, LineActorKind::ZoneInfo)? } {
                if trace_flags & TRACE_ZONE_CHANGES != 0 { break; }
            } else if !source.is_null() && unsafe { ops.is_blocked_by(source, actor)? } {
                if trace_flags & TRACE_BLOCK_ACTORS != 0 { break; }
            } else if trace_flags & TRACE_OTHERS != 0 {
                break;
            }
        }
        check = unsafe { (*check).next.cast() };
    }
    if check.is_null() {
        unsafe { (*hit).time = 1.0; (*hit).actor = std::ptr::null_mut(); }
        Ok(true)
    } else {
        unsafe { hit.write(*check); }
        Ok(false)
    }
}

/// Selects the nearest blocking point result in the native linked-list order.
pub(crate) unsafe fn select_point_hit(
    hit: *mut FCheckResult,
    mut first: *mut FCheckResult,
    location: FVector,
) -> bool {
    if first.is_null() {
        return true;
    }
    unsafe { hit.write(*first); }
    first = unsafe { (*first).next.cast() };
    while !first.is_null() {
        if crate::vec::size_squared(crate::vec::sub(unsafe { (*first).location }, location))
            < crate::vec::size_squared(crate::vec::sub(unsafe { (*hit).location }, location))
        {
            unsafe { hit.write(*first); }
        }
        first = unsafe { (*first).next.cast() };
    }
    false
}

struct CanonicalLineFilterOps<'a> {
    access: CoreAccess<'a>,
}

impl LineFilterOps for CanonicalLineFilterOps<'_> {
    type Error = RuntimeError;

    unsafe fn is_owned_by(&mut self, source: *mut AActor, other: *mut AActor) -> RuntimeResult<bool> {
        Ok(unsafe { actor::is_owned_by(source, other) != 0 })
    }

    unsafe fn is_kind(&mut self, actor: *mut AActor, kind: LineActorKind) -> RuntimeResult<bool> {
        let name = match kind {
            LineActorKind::LevelInfo => "Engine.LevelInfo",
            LineActorKind::Mover => "Engine.Mover",
            LineActorKind::ZoneInfo => "Engine.ZoneInfo",
        };
        unsafe { is_actor_kind(self.access, actor, name) }
    }

    unsafe fn is_blocked_by(&mut self, source: *mut AActor, other: *mut AActor) -> RuntimeResult<bool> {
        unsafe { actor_blocks(self.access, source, other) }
    }
}

struct CanonicalMultiLineOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
}

impl MultiLineOps for CanonicalMultiLineOps<'_> {
    type Error = RuntimeError;

    unsafe fn world_line_check(
        &mut self,
        level_info: *mut c_void,
        hit: *mut FCheckResult,
        end: FVector,
        start: FVector,
        extent: FVector,
        flags: u32,
    ) -> RuntimeResult<bool> {
        if level_info.is_null() { return Ok(true); }
        let model = unsafe { (*self.level).model.cast::<UModel>() };
        if model.is_null() { return Ok(true); }
        unsafe { collision_owner::world_line_check(self.access, model, hit, end, start, extent, flags) }
    }

    unsafe fn actor_line_check(
        &mut self,
        _memory: *mut c_void,
        end: FVector,
        start: FVector,
        extent: FVector,
        flags: u32,
    ) -> RuntimeResult<*mut FCheckResult> {
        unsafe {
            crate::level_owner::hash_line_check(
                self.access,
                self.level,
                self.access.scratch_memory(),
                end,
                start,
                extent,
                flags,
            )
        }
    }

    unsafe fn is_mover(&mut self, actor: *mut AActor) -> RuntimeResult<bool> {
        unsafe { is_actor_kind(self.access, actor, "Engine.Mover") }
    }

    unsafe fn alloc_results(&mut self, _memory: *mut c_void, count: usize) -> RuntimeResult<*mut FCheckResult> {
        let bytes = (count * std::mem::size_of::<FCheckResult>()) as i32;
        let alignment = std::mem::align_of::<FCheckResult>() as i32;
        unsafe { self.access.scratch_memory().push_bytes(bytes, alignment) }
            .map(|memory| memory.cast())
    }
}

struct CanonicalMultiPointOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
}

impl MultiPointOps for CanonicalMultiPointOps<'_> {
    type Error = RuntimeError;

    unsafe fn actor_point_check(
        &mut self,
        _memory: *mut c_void,
        location: FVector,
        extent: FVector,
        _extra_flags: u32,
    ) -> RuntimeResult<*mut FCheckResult> {
        unsafe { crate::level_owner::hash_point_check(self.access, self.level, location, extent) }
    }

    unsafe fn world_point_check(
        &mut self,
        level_info: *mut c_void,
        hit: *mut FCheckResult,
        location: FVector,
        extent: FVector,
        extra_flags: u32,
    ) -> RuntimeResult<bool> {
        if level_info.is_null() { return Ok(true); }
        let model = unsafe { (*self.level).model.cast::<UModel>() };
        if model.is_null() { return Ok(true); }
        unsafe { collision_owner::world_point_check(self.access, model, hit, location, extent, extra_flags) }
    }

    unsafe fn alloc_result(
        &mut self,
        _memory: *mut c_void,
        result: FCheckResult,
    ) -> RuntimeResult<*mut FCheckResult> {
        let memory = unsafe {
            self.access.scratch_memory().push_bytes(
                std::mem::size_of::<FCheckResult>() as i32,
                std::mem::align_of::<FCheckResult>() as i32,
            )?
        };
        unsafe { memory.cast::<FCheckResult>().write(result); }
        Ok(memory.cast())
    }
}

/// Performs canonical `ULevel::SinglePointCheck` using world BSP and actor collision.
///
/// # Safety
/// `level`, `hit`, and any LevelInfo Actor must be live canonical records owned by `access`.
pub unsafe fn single_point_check(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    hit: *mut FCheckResult,
    location: FVector,
    extent: FVector,
    extra_flags: u32,
    level_info: *mut AActor,
    check_actors: bool,
) -> RuntimeResult<bool> {
    if level.is_null() || hit.is_null() {
        return Err(RuntimeError::new("Invalid canonical SinglePointCheck arguments"));
    }
    let memory = access.scratch_memory();
    // SAFETY: the mark bounds all actor-hash results to this synchronous query.
    let mark = unsafe { memory.mark() };
    let result = unsafe {
        single_point_check_body(access, level, hit, location, extent, extra_flags, level_info, check_actors)
    };
    // SAFETY: the result hit is copied to caller-owned storage before releasing scratch results.
    unsafe { memory.pop(&mark); }
    result
}

unsafe fn single_point_check_body(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    hit: *mut FCheckResult,
    location: FVector,
    extent: FVector,
    extra_flags: u32,
    level_info: *mut AActor,
    check_actors: bool,
) -> RuntimeResult<bool> {
    let mut ops = CanonicalMultiPointOps { access, level };
    let first = unsafe {
        multi_point_check_with_ops(
            &mut ops,
            access.scratch_memory().as_ptr().cast(),
            level_info.cast(),
            location,
            extent,
            extra_flags,
            check_actors,
        )?
    };
    Ok(unsafe { select_point_hit(hit, first, location) })
}

/// Runs canonical `ULevel::MultiLineCheck` and returns its scratch-owned hit list.
///
/// # Safety
/// The level, optional LevelInfo and all reached actors remain live until the
/// caller's surrounding scratch mark is popped.
pub(crate) unsafe fn multi_line_check(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    level_info: *mut AActor,
    end: FVector,
    start: FVector,
    extent: FVector,
    check_actors: bool,
    extra_flags: u32,
) -> RuntimeResult<*mut FCheckResult> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical Level in MultiLineCheck"));
    }
    let mut ops = CanonicalMultiLineOps { access, level };
    unsafe {
        multi_line_check_with_ops(
            &mut ops,
            access.scratch_memory().as_ptr().cast(),
            level_info.cast(),
            end,
            start,
            extent,
            check_actors,
            extra_flags,
        )
    }
}

/// Performs canonical `ULevel::SingleLineCheck`, including BSP, actor ordering and source filters.
///
/// # Safety
/// `level`, `hit`, and every non-null source or hit Actor must be live canonical records.
pub unsafe fn single_line_check(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    hit: *mut FCheckResult,
    source: *mut AActor,
    end: FVector,
    start: FVector,
    trace_flags: u32,
    extent: FVector,
    extra_flags: u32,
) -> RuntimeResult<bool> {
    if level.is_null() || hit.is_null() {
        return Err(RuntimeError::new("Invalid canonical SingleLineCheck arguments"));
    }
    let memory = access.scratch_memory();
    // SAFETY: the mark bounds actor-hash results to this synchronous query.
    let mark = unsafe { memory.mark() };
    let result = unsafe {
        single_line_check_body(access, level, hit, source, end, start, trace_flags, extent, extra_flags)
    };
    // SAFETY: the selected hit is copied to caller-owned storage before releasing query links.
    unsafe { memory.pop(&mark); }
    result
}

unsafe fn single_line_check_body(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    hit: *mut FCheckResult,
    source: *mut AActor,
    end: FVector,
    start: FVector,
    trace_flags: u32,
    extent: FVector,
    extra_flags: u32,
) -> RuntimeResult<bool> {
    let level_info = if trace_flags & TRACE_LEVEL != 0 {
        let array = unsafe {
            std::ptr::addr_of!((*level).base.actors)
                .cast::<hp2_abi::engine::generated::TArrayRaw>()
                .read_unaligned()
        };
        if array.data.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { array.data.cast::<*mut AActor>().read_unaligned() }
        }
    } else {
        std::ptr::null_mut()
    };
    let mut ops = CanonicalMultiLineOps { access, level };
    let first = unsafe {
        multi_line_check_with_ops(
            &mut ops,
            access.scratch_memory().as_ptr().cast(),
            level_info.cast(),
            end,
            start,
            extent,
            trace_flags & (TRACE_BLOCK_ACTORS | TRACE_MOVERS | TRACE_LEVEL | TRACE_OTHERS) != 0,
            extra_flags,
        )?
    };
    unsafe {
        filter_line_hits_with_ops(
            &mut CanonicalLineFilterOps { access },
            hit,
            source,
            first,
            trace_flags,
        )
    }
}

unsafe fn is_actor_kind(access: CoreAccess<'_>, actor: *mut AActor, name: &str) -> RuntimeResult<bool> {
    let class = access.native_class(name)?;
    Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
}

unsafe fn actor_blocks(access: CoreAccess<'_>, source: *mut AActor, other: *mut AActor) -> RuntimeResult<bool> {
    unsafe {
        actor::is_blocked_by_with(source, other, |value, kind| {
            let name = match kind {
                actor::class::ABRUSH => "Engine.Brush",
                actor::class::APLAYERPAWN => "Engine.PlayerPawn",
                actor::class::APROJECTILE => "Engine.Projectile",
                _ => return Err(RuntimeError::new("Unknown Actor class in blocking trace")),
            };
            let class = access.native_class(name)?;
            Ok(hp2_core::object_model::is_a(value, class.cast()))
        })
    }
}


