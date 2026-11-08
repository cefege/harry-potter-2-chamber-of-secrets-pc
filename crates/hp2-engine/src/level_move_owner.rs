//! Canonical `ULevel::MoveActor` policy and backend adapters.

use hp2_abi::FVector;
use hp2_abi::engine::generated::{AActor, FCheckResult, FRotator, ULevel};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, ptr};

pub(crate) trait MoveActorOps {
    type Error;
    unsafe fn is_editor(&mut self) -> Result<bool, Self::Error>;
    type Mark;
    unsafe fn mark_push(&mut self) -> Result<Self::Mark, Self::Error>;
    unsafe fn mark_pop(&mut self, mark: &Self::Mark) -> Result<(), Self::Error>;
    unsafe fn app_cycles(&mut self) -> Result<u32, Self::Error>;
    unsafe fn actor_collision_box(
        &mut self,
        actor: *mut AActor,
    ) -> Result<(FVector, FVector), Self::Error>;
    unsafe fn is_moving_brush(&mut self, actor: *mut AActor) -> Result<bool, Self::Error>;
    unsafe fn has_hash(&mut self, level: *mut ULevel) -> Result<bool, Self::Error>;
    unsafe fn is_kind(
        &mut self,
        actor: *mut AActor,
        kind: MoveActorKind,
    ) -> Result<bool, Self::Error>;
    unsafe fn multi_line_check(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        end: FVector,
        start: FVector,
        extent: FVector,
        check_actors: bool,
        level_info: *mut AActor,
    ) -> Result<*mut FCheckResult, Self::Error>;
    unsafe fn is_blocked_by(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error>;
    unsafe fn actor_count(&mut self, level: *mut ULevel) -> Result<i32, Self::Error>;
    unsafe fn actor_at(
        &mut self,
        level: *mut ULevel,
        index: i32,
    ) -> Result<*mut AActor, Self::Error>;
    unsafe fn sin_cos(&mut self, angle: i32) -> Result<(f32, f32), Self::Error>;
    unsafe fn move_actor_recursive(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        delta: FVector,
        rotation: FRotator,
        hit: *mut FCheckResult,
        test: bool,
        ignore_pawns: bool,
        ignore_bases: bool,
        no_fail: bool,
    ) -> Result<bool, Self::Error>;
    unsafe fn check_encroachment(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        location: FVector,
        rotation: FRotator,
    ) -> Result<bool, Self::Error>;
    unsafe fn hash_remove(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
    ) -> Result<(), Self::Error>;
    unsafe fn hash_add(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
    ) -> Result<(), Self::Error>;
    unsafe fn bump_observer(
        &mut self,
        recipient: *mut AActor,
        other: *mut AActor,
    ) -> Result<(), Self::Error>;
    unsafe fn event_bump(
        &mut self,
        recipient: *mut AActor,
        other: *mut AActor,
    ) -> Result<(), Self::Error>;
    unsafe fn begin_touch(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<(), Self::Error>;
    unsafe fn is_overlapping(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> Result<bool, Self::Error>;
    unsafe fn end_touch(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
        no_notify: bool,
    ) -> Result<(), Self::Error>;
    unsafe fn set_actor_zone(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        test: bool,
    ) -> Result<(), Self::Error>;
    unsafe fn tracker_update(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
    ) -> Result<(), Self::Error>;
}

pub(crate) trait FarMoveActorOps {
    type Error;
    unsafe fn is_editor(&mut self) -> Result<bool, Self::Error>;
    unsafe fn has_hash(&mut self, level: *mut ULevel) -> Result<bool, Self::Error>;
    unsafe fn actor_collision_extent(&mut self, actor: *mut AActor) -> Result<FVector, Self::Error>;
    unsafe fn find_spot(&mut self, level: *mut ULevel, extent: FVector, location: *mut FVector) -> Result<bool, Self::Error>;
    unsafe fn level_info(&mut self, level: *mut ULevel) -> Result<*mut AActor, Self::Error>;
    unsafe fn net_mode(&mut self, level_info: *mut AActor) -> Result<i32, Self::Error>;
    unsafe fn check_encroachment(&mut self, level: *mut ULevel, actor: *mut AActor, location: FVector, rotation: FRotator) -> Result<bool, Self::Error>;
    unsafe fn actor_count(&mut self, level: *mut ULevel) -> Result<i32, Self::Error>;
    unsafe fn actor_at(&mut self, level: *mut ULevel, index: i32) -> Result<*mut AActor, Self::Error>;
    unsafe fn set_base(&mut self, actor: *mut AActor, new_base: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn hash_remove(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn hash_add(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn tracker_update(&mut self, level: *mut ULevel, actor: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn set_actor_zone(&mut self, level: *mut ULevel, actor: *mut AActor, test: bool) -> Result<(), Self::Error>;
}

/// Runs the FarMoveActor state transition against backend-provided Engine operations.
///
/// # Safety
/// The level, actor, and every object reached by backend operations must be live.
pub(crate) unsafe fn far_move_actor_with_ops<O: FarMoveActorOps>(
    ops: &mut O,
    level: *mut ULevel,
    actor: *mut AActor,
    destination: FVector,
    test: bool,
    no_check: bool,
) -> Result<bool, O::Error> {
    let is_editor = unsafe { ops.is_editor()? };
    if (unsafe { (*actor).b_static() || !(*actor).b_movable() }) && !is_editor {
        return Ok(false);
    }
    if unsafe { (*actor).b_collide_actors() } && unsafe { ops.has_hash(level)? } {
        unsafe { ops.hash_remove(level, actor)?; }
    }
    let level_info = unsafe { ops.level_info(level)? };
    let mut location = destination;
    let mut result = true;
    if !no_check
        && (unsafe { (*actor).b_collide_world() }
            || (unsafe { (*actor).b_collide_when_placing() }
                && unsafe { ops.net_mode(level_info)? } != 3))
    {
        let extent = unsafe { ops.actor_collision_extent(actor)? };
        result = unsafe { ops.find_spot(level, extent, &mut location)? };
    }
    if result && !test && !no_check {
        let rotation = unsafe { ptr::addr_of!((*actor).rotation).read_unaligned() };
        result = unsafe { ops.check_encroachment(level, actor, location, rotation)? };
    }
    if result {
        if !test && unsafe { ops.net_mode(level_info)? } != 3 {
            if unsafe { (*actor).standing_count } > 0 {
                let mut index = 0;
                while index < unsafe { ops.actor_count(level)? } {
                    let other = unsafe { ops.actor_at(level, index)? };
                    if !other.is_null()
                        && unsafe { ptr::addr_of!((*other).base).read_unaligned() == actor.cast() }
                    {
                        unsafe { ops.set_base(other, ptr::null_mut())?; }
                    }
                    index += 1;
                }
            }
            unsafe { (*actor).set_b_just_teleported(true); }
        }
        unsafe {
            ptr::addr_of_mut!((*actor).location).write_unaligned(location);
            ptr::addr_of_mut!((*actor).old_location).write_unaligned(location);
        }
    }
    if unsafe { (*actor).b_collide_actors() } && unsafe { ops.has_hash(level)? } {
        unsafe { ops.hash_add(level, actor)?; }
    }
    unsafe { ops.tracker_update(level, actor)?; }
    if result {
        unsafe { ops.set_actor_zone(level, actor, test)?; }
    }
    Ok(result)
}

/// Applies FarMoveActor with Core-owned collision, relation, hash, and brush-tracker operations.
///
/// # Safety
/// `level` and `actor` are live canonical objects owned by `access`.
pub unsafe fn far_move_actor(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    actor: *mut AActor,
    destination: FVector,
    test: bool,
    no_check: bool,
) -> RuntimeResult<bool> {
    if level.is_null() || actor.is_null() {
        return Err(RuntimeError::new("Invalid canonical FarMoveActor arguments"));
    }
    unsafe { far_move_actor_with_ops(&mut CanonicalFarMoveOps { access }, level, actor, destination, test, no_check) }
}

struct CanonicalFarMoveOps<'a> {
    access: CoreAccess<'a>,
}

impl FarMoveActorOps for CanonicalFarMoveOps<'_> {
    type Error = RuntimeError;

    unsafe fn is_editor(&mut self) -> RuntimeResult<bool> {
        Ok(self.access.process_mode().is_editor)
    }
    unsafe fn has_hash(&mut self, level: *mut ULevel) -> RuntimeResult<bool> {
        Ok(!unsafe { (*level).hash }.is_null())
    }
    unsafe fn actor_collision_extent(&mut self, actor: *mut AActor) -> RuntimeResult<FVector> {
        unsafe { crate::collision_owner::actor_cylinder_extent(self.access, actor) }
    }
    unsafe fn find_spot(&mut self, level: *mut ULevel, extent: FVector, location: *mut FVector) -> RuntimeResult<bool> {
        unsafe { crate::level_zone_owner::find_spot(self.access, level, extent, location, false, false) }
    }
    unsafe fn level_info(&mut self, level: *mut ULevel) -> RuntimeResult<*mut AActor> {
        let info = unsafe { crate::level_owner::actor_at(level, 0) };
        if info.is_null() {
            return Ok(info);
        }
        let class = self.access.native_class("Engine.LevelInfo")?;
        if !unsafe { hp2_core::object_model::is_a(info.cast(), class.cast()) } {
            return Err(RuntimeError::new("FarMoveActor Level actor zero is not LevelInfo"));
        }
        Ok(info)
    }
    unsafe fn net_mode(&mut self, level_info: *mut AActor) -> RuntimeResult<i32> {
        if level_info.is_null() {
            return Err(RuntimeError::new("FarMoveActor Level has no LevelInfo"));
        }
        Ok(unsafe { (*level_info.cast::<hp2_abi::engine::generated::ALevelInfo>()).net_mode as i32 })
    }
    unsafe fn check_encroachment(&mut self, level: *mut ULevel, actor: *mut AActor, location: FVector, rotation: FRotator) -> RuntimeResult<bool> {
        let encroached = unsafe {
            crate::level_encroachment_owner::check_encroachment(
                self.access, level, actor, location, rotation, true,
            )?
        };
        Ok(!encroached)
    }
    unsafe fn actor_count(&mut self, level: *mut ULevel) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(level) }
    }
    unsafe fn actor_at(&mut self, level: *mut ULevel, index: i32) -> RuntimeResult<*mut AActor> {
        unsafe { Ok(crate::level_owner::actor_at(level, index)) }
    }
    unsafe fn set_base(&mut self, actor: *mut AActor, new_base: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_lifecycle_owner::set_base(self.access, actor, new_base, true) }
    }
    unsafe fn hash_remove(&mut self, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_remove(self.access, level, actor, false) }
    }
    unsafe fn hash_add(&mut self, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_add(self.access, level, actor) }
    }
    unsafe fn tracker_update(&mut self, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::dynbsp_owner::update_moving_brush_tracker(self.access, level, actor) }
    }
    unsafe fn set_actor_zone(&mut self, level: *mut ULevel, actor: *mut AActor, test: bool) -> RuntimeResult<()> {
        unsafe { crate::level_zone_owner::set_actor_zone(self.access, level, actor, test, false) }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum MoveActorKind {
    Mover,
    Pawn,
    Decoration,
}

/// Runs the native MoveActor state transition against backend-provided engine leaves.
///
/// # Safety
/// The level, actor, hit, and every object reached by backend operations must be live.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn move_actor_with_ops<O: MoveActorOps>(
    ops: &mut O,
    level: *mut ULevel,
    actor: *mut AActor,
    delta: FVector,
    new_rotation: FRotator,
    hit: *mut FCheckResult,
    test: bool,
    ignore_pawns: bool,
    ignore_bases: bool,
    no_fail: bool,
) -> Result<bool, O::Error> {
    macro_rules! field {
        ($name:ident) => {
            unsafe { std::ptr::addr_of!((*actor).$name).read_unaligned() }
        };
    }
    macro_rules! method {
        ($name:ident()) => {
            unsafe { (*actor).$name() }
        };
    }
    if (method!(b_static()) || !method!(b_movable())) && !unsafe { ops.is_editor()? } {
        return Ok(false);
    }
    let rotation = field!(rotation);
    let moving_brush = unsafe { ops.is_moving_brush(actor)? };
    if nearly_zero(delta) {
        if rot_eq(new_rotation, rotation) {
            return Ok(true);
        }
        if field!(standing_count) == 0 && !moving_brush {
            unsafe {
                std::ptr::addr_of_mut!((*actor).rotation).write_unaligned(new_rotation);
            }
            return Ok(true);
        }
    }
    unsafe {
        hit.write(crate::levact::check_result(1.0));
    }
    unsafe {
        (*level).num_moves = (*level).num_moves.wrapping_add(1);
    }
    let cycles = unsafe { ops.app_cycles()? };
    unsafe {
        (*level).move_cycles = (*level).move_cycles.wrapping_sub(cycles as i32);
    }
    let mark = unsafe { ops.mark_push()? };
    let mut completed = false;
    let result = (|| -> Result<bool, O::Error> {
        let delta_size = if nearly_zero(delta) {
            0.0
        } else {
            crate::vec::size(delta)
        };
        let delta_dir = if nearly_zero(delta) {
            delta
        } else {
            crate::vec::div(delta, delta_size)
        };
        let test_adjust = if moving_brush { 0.0 } else { 2.0 };
        let test_delta = crate::vec::add(delta, crate::vec::scale(delta_dir, test_adjust));
        let mut maybe_touched = false;
        let mut first_hit = std::ptr::null_mut();
        let collide_actors = method!(b_collide_actors());
        let collide_world = method!(b_collide_world());
        if (collide_actors || collide_world) && (delta.x != 0.0 || delta.y != 0.0 || delta.z != 0.0)
        {
            let (min, max) = unsafe { ops.actor_collision_box(actor)? };
            let trace_location = crate::vec::scale(crate::vec::add(min, max), 0.5);
            let mut extent = crate::vec::scale(crate::vec::sub(max, min), 0.5);
            if moving_brush {
                extent = crate::vec::sub(
                    extent,
                    FVector {
                        x: 0.51,
                        y: 0.51,
                        z: 0.51,
                    },
                );
            }
            let is_mover = unsafe { ops.is_kind(actor, MoveActorKind::Mover)? };
            let encroach_type = if is_mover {
                unsafe {
                    std::ptr::addr_of!(
                        (*actor.cast::<hp2_abi::engine::generated::AMover>()).mover_encroach_type
                    )
                    .read_unaligned()
                }
            } else {
                0
            };
            if crate::levact::do_multi_line_check(is_mover, encroach_type) {
                let level_info = if collide_world {
                    unsafe { first_level_info(ops, level)? }
                } else {
                    std::ptr::null_mut()
                };
                first_hit = unsafe {
                    ops.multi_line_check(
                        level,
                        actor,
                        crate::vec::add(trace_location, test_delta),
                        trace_location,
                        extent,
                        collide_actors,
                        level_info,
                    )?
                };
            }
        }
        if collide_world || method!(b_block_actors()) || method!(b_block_players()) {
            let mut check = first_hit;
            while !check.is_null() {
                let other = unsafe { (*check).actor.cast::<AActor>() };
                if (!ignore_pawns
                    || unsafe { (*other).b_static() }
                    || !unsafe {
                        ops.is_kind(other, MoveActorKind::Pawn)?
                            || ops.is_kind(other, MoveActorKind::Decoration)?
                    })
                    && (!ignore_bases || !unsafe { crate::actor::is_based_on(actor, other) })
                    && !unsafe { crate::actor::is_based_on(other, actor) }
                {
                    maybe_touched = true;
                    if unsafe { ops.is_blocked_by(actor, other)? } {
                        unsafe {
                            hit.write(*check);
                        }
                        break;
                    }
                }
                check = unsafe { (*check).next.cast() };
            }
        }
        let mut final_delta = delta;
        if unsafe { (*hit).time } < 1.0 && !no_fail {
            let final_delta_size = (delta_size + test_adjust) * unsafe { (*hit).time };
            unsafe {
                (*hit).time = (final_delta_size - test_adjust) / delta_size;
            }
            if unsafe { (*hit).time } <= 1.0e-4 {
                unsafe {
                    (*hit).time = 0.0;
                }
                return Ok(false);
            }
            final_delta = crate::vec::scale(delta, unsafe { (*hit).time });
        }
        if field!(standing_count) != 0 && !test {
            let mut i = 0;
            while i < unsafe { ops.actor_count(level)? } {
                let other = unsafe { ops.actor_at(level, i)? };
                i += 1;
                if other.is_null() || unsafe { (*other).base } != actor.cast() {
                    continue;
                }
                let rotation = field!(rotation);
                let yaw_delta = new_rotation.yaw.wrapping_sub(rotation.yaw);
                let delta_rot = FRotator {
                    pitch: 0,
                    yaw: yaw_delta,
                    roll: 0,
                };
                let mut rot_motion = crate::vec::zero();
                if !rot_eq(new_rotation, rotation) {
                    let reduced = FRotator {
                        pitch: 0,
                        yaw: (new_rotation.yaw & 0xfffc).wrapping_sub(rotation.yaw & 0xfffc),
                        roll: 0,
                    };
                    let pointer = crate::vec::sub(field!(location), unsafe { (*other).location });
                    let (sin, cos) = unsafe { ops.sin_cos(reduced.yaw)? };
                    rot_motion = crate::vec::sub(pointer, rotate_yaw(pointer, sin, cos));
                }
                let other_rotation =
                    unsafe { std::ptr::addr_of!((*other).rotation).read_unaligned() };
                let mut local_hit = crate::levact::check_result(1.0);
                let move_delta = crate::vec::add(final_delta, rot_motion);
                let move_rotation = FRotator {
                    pitch: other_rotation.pitch,
                    yaw: other_rotation.yaw.wrapping_add(delta_rot.yaw),
                    roll: other_rotation.roll,
                };
                let _ = unsafe {
                    ops.move_actor_recursive(
                        level,
                        other,
                        move_delta,
                        move_rotation,
                        &mut local_hit,
                        false,
                        false,
                        true,
                        false,
                    )?
                };
                if unsafe { ops.is_kind(other, MoveActorKind::Pawn)? } {
                    let pawn = other.cast::<hp2_abi::engine::generated::APawn>();
                    let view =
                        unsafe { std::ptr::addr_of!((*pawn).view_rotation).read_unaligned() };
                    unsafe {
                        std::ptr::addr_of_mut!((*pawn).view_rotation).write_unaligned(FRotator {
                            pitch: view.pitch,
                            yaw: view.yaw.wrapping_add(delta_rot.yaw),
                            roll: view.roll,
                        });
                    }
                }
            }
        }
        if collide_actors && unsafe { ops.has_hash(level)? } {
            unsafe {
                ops.hash_remove(level, actor)?;
            }
        }
        let test_location = crate::vec::add(field!(location), final_delta);
        if !test
            && !no_fail
            && !unsafe { ops.is_kind(actor, MoveActorKind::Pawn)? }
            && unsafe { ops.check_encroachment(level, actor, test_location, new_rotation)? }
        {
            let now = unsafe { ops.app_cycles()? };
            unsafe {
                (*level).move_cycles = (*level)
                    .move_cycles
                    .wrapping_add(now.wrapping_sub(34) as i32);
            }
            if collide_actors && unsafe { ops.has_hash(level)? } {
                unsafe {
                    ops.hash_add(level, actor)?;
                }
            }
            return Ok(false);
        }
        let location = crate::vec::add(field!(location), final_delta);
        unsafe {
            std::ptr::addr_of_mut!((*actor).location).write_unaligned(location);
            std::ptr::addr_of_mut!((*actor).rotation).write_unaligned(new_rotation);
        }
        if collide_actors && unsafe { ops.has_hash(level)? } {
            unsafe {
                ops.hash_add(level, actor)?;
            }
        }
        unsafe {
            ops.tracker_update(level, actor)?;
        }
        if !test {
            let hit_actor = unsafe { (*hit).actor.cast::<AActor>() };
            if !hit_actor.is_null()
                && hit_actor.cast::<c_void>() != unsafe { first_level_info(ops, level)? }.cast()
                && !unsafe { crate::actor::is_based_on(actor, hit_actor) }
            {
                unsafe {
                    ops.bump_observer(hit_actor, actor)?;
                    ops.event_bump(hit_actor, actor)?;
                    ops.bump_observer(actor, hit_actor)?;
                    ops.event_bump(actor, hit_actor)?;
                }
            }
            if maybe_touched || !method!(b_block_actors()) || !method!(b_block_players()) {
                let mut check = first_hit;
                while !check.is_null() && unsafe { (*check).time < (*hit).time } {
                    let other = unsafe { (*check).actor.cast::<AActor>() };
                    if !unsafe { crate::actor::is_based_on(other, actor) }
                        && (!ignore_bases || !unsafe { crate::actor::is_based_on(actor, other) })
                        && !unsafe { ops.is_blocked_by(actor, other)? }
                    {
                        unsafe {
                            ops.begin_touch(actor, other)?;
                        }
                    }
                    check = unsafe { (*check).next.cast() };
                }
            }
            for i in 0..4 {
                let touching = unsafe {
                    std::ptr::addr_of!((*actor).touching)
                        .cast::<*mut c_void>()
                        .add(i)
                        .read_unaligned()
                        .cast::<AActor>()
                };
                if !touching.is_null() && !unsafe { ops.is_overlapping(actor, touching)? } {
                    unsafe {
                        ops.end_touch(actor, touching, false)?;
                    }
                }
            }
        }
        unsafe {
            ops.set_actor_zone(level, actor, test)?;
        }
        completed = true;
        Ok(unsafe { (*hit).time > 0.0 })
    })();
    unsafe {
        ops.mark_pop(&mark)?;
    }
    let moved = result?;
    if completed {
        let now = unsafe { ops.app_cycles()? };
        unsafe {
            (*level).move_cycles = (*level)
                .move_cycles
                .wrapping_add(now.wrapping_sub(34) as i32);
        }
    }
    Ok(moved)
}

pub unsafe fn move_actor(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    actor: *mut AActor,
    delta: FVector,
    rotation: FRotator,
    hit: *mut FCheckResult,
    test: bool,
    ignore_pawns: bool,
    ignore_bases: bool,
    no_fail: bool,
) -> RuntimeResult<bool> {
    if level.is_null() || actor.is_null() || hit.is_null() {
        return Err(RuntimeError::new("Invalid canonical MoveActor arguments"));
    }
    let mut ops = CanonicalMoveActorOps { access };
    unsafe {
        move_actor_with_ops(
            &mut ops,
            level,
            actor,
            delta,
            rotation,
            hit,
            test,
            ignore_pawns,
            ignore_bases,
            no_fail,
        )
    }
}

struct CanonicalMoveActorOps<'a> {
    access: CoreAccess<'a>,
}

impl MoveActorOps for CanonicalMoveActorOps<'_> {
    type Error = RuntimeError;
    type Mark = hp2_abi::render_span::FMemMark;
    unsafe fn mark_push(&mut self) -> RuntimeResult<Self::Mark> {
        Ok(unsafe { self.access.scratch_memory().mark() })
    }
    unsafe fn mark_pop(&mut self, mark: &Self::Mark) -> RuntimeResult<()> {
        unsafe {
            self.access.scratch_memory().pop(mark);
        }
        Ok(())
    }
    unsafe fn is_editor(&mut self) -> RuntimeResult<bool> {
        Ok(self.access.process_mode().is_editor)
    }
    unsafe fn app_cycles(&mut self) -> RuntimeResult<u32> {
        Ok(hp2_core::unix_clock::cycles())
    }
    unsafe fn is_moving_brush(&mut self, actor: *mut AActor) -> RuntimeResult<bool> {
        let brush = unsafe { (*actor).brush };
        if brush.is_null() || unsafe { (*actor).b_static() } {
            return Ok(false);
        }
        let class = self.access.native_class("Engine.Brush")?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }
    unsafe fn has_hash(&mut self, level: *mut ULevel) -> RuntimeResult<bool> {
        Ok(unsafe { !(*level).hash.is_null() })
    }
    unsafe fn actor_collision_box(
        &mut self,
        actor: *mut AActor,
    ) -> RuntimeResult<(FVector, FVector)> {
        let bx = unsafe { crate::collision_owner::actor_collision_box(self.access, actor)? };
        Ok((bx.min, bx.max))
    }
    unsafe fn is_kind(&mut self, actor: *mut AActor, kind: MoveActorKind) -> RuntimeResult<bool> {
        let class = match kind {
            MoveActorKind::Mover => "Engine.Mover",
            MoveActorKind::Pawn => "Engine.Pawn",
            MoveActorKind::Decoration => "Engine.Decoration",
        };
        let class = self.access.native_class(class)?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }
    unsafe fn multi_line_check(
        &mut self,
        level: *mut ULevel,
        _actor: *mut AActor,
        end: FVector,
        start: FVector,
        extent: FVector,
        check_actors: bool,
        level_info: *mut AActor,
    ) -> RuntimeResult<*mut FCheckResult> {
        unsafe {
            crate::level_trace_owner::multi_line_check(
                self.access,
                level,
                level_info,
                end,
                start,
                extent,
                check_actors,
                0,
            )
        }
    }
    unsafe fn is_blocked_by(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> RuntimeResult<bool> {
        unsafe {
            crate::actor::is_blocked_by_with(actor, other, |obj, id| {
                let name = match id {
                    crate::actor::class::ABRUSH => "Engine.Brush",
                    crate::actor::class::APLAYERPAWN => "Engine.PlayerPawn",
                    crate::actor::class::APROJECTILE => "Engine.Projectile",
                    _ => return Err(RuntimeError::new("Unsupported class in IsBlockedBy")),
                };
                let c = self.access.native_class(name)?;
                Ok(hp2_core::object_model::is_a(obj, c.cast()))
            })
        }
    }
    unsafe fn actor_count(&mut self, level: *mut ULevel) -> RuntimeResult<i32> {
        unsafe { crate::level_owner::actor_count(level) }
    }
    unsafe fn actor_at(&mut self, level: *mut ULevel, index: i32) -> RuntimeResult<*mut AActor> {
        Ok(unsafe { crate::level_owner::actor_at(level, index) })
    }
    unsafe fn sin_cos(&mut self, angle: i32) -> RuntimeResult<(f32, f32)> {
        let table = hp2_core::math::trig_table();
        Ok((
            hp2_core::math::sin_tab_inline(angle, table),
            hp2_core::math::cos_tab_inline(angle, table),
        ))
    }
    unsafe fn move_actor_recursive(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        delta: FVector,
        rotation: FRotator,
        hit: *mut FCheckResult,
        test: bool,
        ignore_pawns: bool,
        ignore_bases: bool,
        no_fail: bool,
    ) -> RuntimeResult<bool> {
        unsafe {
            move_actor(
                self.access,
                level,
                actor,
                delta,
                rotation,
                hit,
                test,
                ignore_pawns,
                ignore_bases,
                no_fail,
            )
        }
    }
    unsafe fn check_encroachment(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        location: FVector,
        rotation: FRotator,
    ) -> RuntimeResult<bool> {
        unsafe {
            crate::level_encroachment_owner::check_encroachment(
                self.access,
                level,
                actor,
                location,
                rotation,
                false,
            )
        }
    }
    unsafe fn hash_remove(&mut self, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_remove(self.access, level, actor, false) }
    }
    unsafe fn hash_add(&mut self, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_owner::hash_add(self.access, level, actor) }
    }
    unsafe fn bump_observer(
        &mut self,
        recipient: *mut AActor,
        other: *mut AActor,
    ) -> RuntimeResult<()> {
        crate::core_owner::observe_actor_movement(
            self.access,
            crate::core_owner::ActorMovementEvent::Bump,
            recipient,
            other,
        )
    }
    unsafe fn event_bump(
        &mut self,
        recipient: *mut AActor,
        other: *mut AActor,
    ) -> RuntimeResult<()> {
        execute_actor_pair(self.access, recipient, "Bump", other)
    }
    unsafe fn begin_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::level_encroachment_owner::begin_touch(self.access, actor, other) }
    }
    unsafe fn is_overlapping(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
    ) -> RuntimeResult<bool> {
        Ok(unsafe {
            crate::actor::is_overlapping_with(actor, other, |obj| {
                self.access
                    .native_class("Engine.Brush")
                    .map(|c| hp2_core::object_model::is_a(obj.cast(), c.cast()))
            })? != 0
        })
    }
    unsafe fn end_touch(
        &mut self,
        actor: *mut AActor,
        other: *mut AActor,
        no_notify: bool,
    ) -> RuntimeResult<()> {
        let mut relation = CanonicalMoveRelationOps {
            access: self.access,
        };
        unsafe { crate::actor::end_touch_with_ops(&mut relation, actor, other, no_notify as i32) }
    }
    unsafe fn set_actor_zone(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
        test: bool,
    ) -> RuntimeResult<()> {
        unsafe { crate::level_zone_owner::set_actor_zone(self.access, level, actor, test, false) }
    }
    unsafe fn tracker_update(
        &mut self,
        level: *mut ULevel,
        actor: *mut AActor,
    ) -> RuntimeResult<()> {
        unsafe { crate::dynbsp_owner::update_moving_brush_tracker(self.access, level, actor) }
    }
}

struct CanonicalMoveRelationOps<'a> {
    access: CoreAccess<'a>,
}
impl crate::actor::ActorRelationOps for CanonicalMoveRelationOps<'_> {
    type Error = RuntimeError;
    unsafe fn event(
        &mut self,
        event: crate::actor::ActorRelationEvent,
        receiver: *mut c_void,
        other: *mut c_void,
    ) -> RuntimeResult<()> {
        let (kind, name) = match event {
            crate::actor::ActorRelationEvent::UnTouch =>
                (crate::core_owner::ActorMovementEvent::UnTouch, "UnTouch"),
            _ => return Err(RuntimeError::new("Unexpected actor relation event during MoveActor touch update")),
        };
        crate::core_owner::observe_actor_movement(self.access, kind, receiver.cast(), other.cast())?;
        execute_actor_pair(self.access, receiver.cast(), name, other.cast())
    }
}

fn execute_actor_pair(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    event: &str,
    other: *mut AActor,
) -> RuntimeResult<()> {
    let name = access.intern_str(event, hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(actor.cast(), name, false)? };
    if function.is_null() {
        return Ok(());
    }
    let parameter_size = unsafe { usize::from((*function).parms_size) };
    let mut parameters = [0u64; 8];
    if parameter_size < std::mem::size_of::<*mut AActor>()
        || parameter_size > std::mem::size_of_val(&parameters)
    {
        return Err(
            RuntimeError::new("Actor event parameter layout exceeds its fixed frame")
                .with_context(event),
        );
    }
    let parameter_bytes = parameters.as_mut_ptr().cast::<u8>();
    unsafe {
        parameter_bytes.cast::<*mut AActor>().write_unaligned(other);
        access.execute_event(
            actor.cast(),
            function,
            parameter_bytes.cast(),
            std::ptr::null_mut(),
        )
    }
}

fn nearly_zero(v: FVector) -> bool {
    v.x.abs() < 1.0e-8 && v.y.abs() < 1.0e-8 && v.z.abs() < 1.0e-8
}
fn rot_eq(a: FRotator, b: FRotator) -> bool {
    a.pitch == b.pitch && a.yaw == b.yaw && a.roll == b.roll
}
fn rotate_yaw(v: FVector, sin: f32, cos: f32) -> FVector {
    FVector {
        x: v.x * cos - v.y * sin,
        y: v.x * sin + v.y * cos,
        z: v.z,
    }
}
unsafe fn first_level_info<O: MoveActorOps>(
    ops: &mut O,
    level: *mut ULevel,
) -> Result<*mut AActor, O::Error> {
    if unsafe { ops.actor_count(level)? } == 0 {
        Ok(std::ptr::null_mut())
    } else {
        unsafe { ops.actor_at(level, 0) }
    }
}

#[cfg(test)]
mod render_host_contracts {
    use super::*;

    #[test]
    fn moving_brush_checks_actor_class_not_model_class() {
        let owner = crate::render_owner::render_test_support::owner("moving-brush-class");
        let access = owner.access();
        let mover = crate::render_owner::render_test_support::construct(access, "Engine.Mover").cast::<AActor>();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Light").cast::<AActor>();
        let model = crate::render_owner::render_test_support::construct(access, "Engine.Model");
        let mut ops = CanonicalMoveActorOps { access };
        unsafe {
            (*mover).set_b_static(false);
            (*mover).brush = std::ptr::null_mut();
            assert!(!ops.is_moving_brush(mover).unwrap());
            (*mover).brush = model.cast();
            assert!(ops.is_moving_brush(mover).unwrap(), "Mover with a Model is a moving brush");
            (*mover).set_b_static(true);
            assert!(!ops.is_moving_brush(mover).unwrap());
            (*actor).set_b_static(false);
            (*actor).brush = model.cast();
            assert!(!ops.is_moving_brush(actor).unwrap(), "an Actor with a Model is not a Brush");
            (*actor).brush = std::ptr::null_mut();
            (*mover).brush = std::ptr::null_mut();
        }
        owner.shutdown().unwrap();
    }
}
