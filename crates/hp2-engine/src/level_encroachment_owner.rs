//! Canonical Core operations for `ULevel::CheckEncroachment`.

use crate::levact::{self, CheckEncroachmentOps};
use hp2_abi::engine::generated::{AActor, APawn, FCheckResult, FRotator, UClass, UFunction, UObject, ULevel};
use hp2_abi::FVector;
use hp2_abi::render_span::FMemMark;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::c_void, ptr};

struct CanonicalCheckEncroachmentOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
}

impl CanonicalCheckEncroachmentOps<'_> {
    fn is_a(&self, actor: *const AActor, kind: u32) -> RuntimeResult<bool> {
        let name = match kind {
            crate::actor::class::AMOVER => "Engine.Mover",
            crate::actor::class::APAWN => "Engine.Pawn",
            crate::actor::class::APLAYERPAWN => "Engine.PlayerPawn",
            crate::actor::class::APROJECTILE => "Engine.Projectile",
            crate::actor::class::ATRIGGER => "Engine.Trigger",
            crate::actor::class::ABRUSH => "Engine.Brush",
            _ => return Err(RuntimeError::new("Unknown Actor class in CheckEncroachment")),
        };
        let class = self.access.native_class(name)?;
        Ok(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })
    }

    unsafe fn event_function(&self, receiver: *mut AActor, name: &str) -> RuntimeResult<*mut UFunction> {
        let event_name = self.access.intern_str(name, FindName::Find)?;
        unsafe { self.access.find_object_function_by_name(receiver.cast(), event_name, false) }
    }

    unsafe fn event_with_actor(&self, receiver: *mut AActor, name: &str, other: *mut AActor) -> RuntimeResult<()> {
        let function = unsafe { self.event_function(receiver, name)? };
        if function.is_null() { return Err(RuntimeError::new("Required Actor event is missing").with_context(name)); }
        let parameter_size = unsafe { usize::from((*function).parms_size) };
        if parameter_size < std::mem::size_of::<*mut AActor>() || parameter_size > 8 * std::mem::size_of::<u64>() {
            return Err(RuntimeError::new("Actor event parameter layout exceeds its fixed frame"));
        }
        let mut parameters = [0u64; 8];
        let parameter_bytes = parameters.as_mut_ptr().cast::<u8>();
        unsafe { parameter_bytes.cast::<*mut AActor>().write_unaligned(other); }
        unsafe {
            self.access.execute_event(
                receiver.cast(),
                function,
                parameter_bytes.cast(),
                ptr::null_mut(),
            )?;
        }
        Ok(())
    }

    unsafe fn event_with_actor_result(&self, receiver: *mut AActor, name: &str, other: *mut AActor) -> RuntimeResult<bool> {
        let function = unsafe { self.event_function(receiver, name)? };
        if function.is_null() { return Err(RuntimeError::new("Required Actor event is missing").with_context(name)); }
        let parameter_size = unsafe { usize::from((*function).parms_size) };
        let return_offset = unsafe { usize::from((*function).return_value_offset) };
        let frame_size = 8 * std::mem::size_of::<u64>();
        if parameter_size < std::mem::size_of::<*mut AActor>()
            || parameter_size > frame_size
            || return_offset == usize::from(u16::MAX)
            || return_offset.checked_add(std::mem::size_of::<i32>()).map_or(true, |end| end > parameter_size)
        {
            return Err(RuntimeError::new("EncroachingOn event has an invalid parameter layout"));
        }
        let mut parameters = [0u64; 8];
        let parameter_bytes = parameters.as_mut_ptr().cast::<u8>();
        unsafe {
            parameter_bytes.cast::<*mut AActor>().write_unaligned(other);
            self.access.execute_event(
                receiver.cast(),
                function,
                parameter_bytes.cast(),
                ptr::null_mut(),
            )?;
        }
        Ok(unsafe { parameter_bytes.add(return_offset).cast::<i32>().read_unaligned() != 0 })
    }

    unsafe fn end_touch(&self, actor: *mut AActor, other: *mut AActor, no_notify_self: bool) -> RuntimeResult<()> {
        let mut ops = CanonicalActorRelationOps { access: self.access, level: self.level };
        unsafe { crate::actor::end_touch_with_ops(&mut ops, actor, other, i32::from(no_notify_self)) }
    }


    unsafe fn begin_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        unsafe { crate::actor::begin_touch_with_ops(self, actor, other) }
    }
}

impl crate::actor::ActorTouchOps for CanonicalCheckEncroachmentOps<'_> {
    type Error = RuntimeError;
    unsafe fn validate_pair(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        if actor.is_null() || other.is_null() || actor == other {
            return Err(RuntimeError::new("Invalid Actor pair in TouchTo"));
        }
        Ok(())
    }
    unsafe fn validate_touch(&mut self, object: *mut c_void) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new("Actor Touching entry is null"));
        }
        let object = object.cast::<UObject>();
        let index = unsafe { (*object).index };
        if index < 0 || self.access.object_at(index) != object {
            return Err(RuntimeError::new("Actor Touching entry is not a live Core object"));
        }
        Ok(())
    }
    unsafe fn is_a(&mut self, actor: *mut AActor, class_id: u32) -> RuntimeResult<bool> {
        CanonicalCheckEncroachmentOps::is_a(self, actor, class_id)
    }
    unsafe fn is_player(&mut self, actor: *mut AActor) -> RuntimeResult<bool> {
        Ok(unsafe { (*actor.cast::<APawn>()).b_is_player() })
    }
    unsafe fn end_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        CanonicalCheckEncroachmentOps::end_touch(self, actor, other, false)
    }
    unsafe fn event_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        crate::core_owner::observe_actor_movement(
            self.access,
            crate::core_owner::ActorMovementEvent::Touch,
            actor,
            other,
        )?;
        unsafe { self.event_with_actor(actor, "Touch", other) }
    }
}
/// Dispatches the canonical Actor touch relation policy used by encroachment and movement.
///
/// # Safety
/// `actor` and `other` must be live canonical actors.
pub(crate) unsafe fn begin_touch(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    other: *mut AActor,
) -> RuntimeResult<()> {
    let level = unsafe { (*actor).x_level.cast::<ULevel>() };
    let mut helper = CanonicalCheckEncroachmentOps { access, level };
    unsafe { helper.begin_touch(actor, other) }
}


struct CanonicalActorRelationOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
}

impl crate::actor::ActorRelationOps for CanonicalActorRelationOps<'_> {
    type Error = RuntimeError;

    unsafe fn event(
        &mut self,
        event: crate::actor::ActorRelationEvent,
        receiver: *mut c_void,
        related: *mut c_void,
    ) -> RuntimeResult<()> {
        let (name, movement_event) = match event {
            crate::actor::ActorRelationEvent::LostChild => ("LostChild", None),
            crate::actor::ActorRelationEvent::GainedChild => ("GainedChild", None),
            crate::actor::ActorRelationEvent::Detach => ("Detach", None),
            crate::actor::ActorRelationEvent::Attach => ("Attach", None),
            crate::actor::ActorRelationEvent::BaseChange => {
                unsafe { self.access.execute_named_event(receiver.cast(), "BaseChange")?; }
                return Ok(());
            }
            crate::actor::ActorRelationEvent::UnTouch =>
                ("UnTouch", Some(crate::core_owner::ActorMovementEvent::UnTouch)),
        };
        if let Some(movement_event) = movement_event {
            crate::core_owner::observe_actor_movement(
                self.access, movement_event, receiver.cast(), related.cast(),
            )?;
        }
        let helper = CanonicalCheckEncroachmentOps { access: self.access, level: self.level };
        unsafe { helper.event_with_actor(receiver.cast(), name, related.cast()) }
    }
}

impl CheckEncroachmentOps for CanonicalCheckEncroachmentOps<'_> {
    type Error = RuntimeError;
    type Mark = FMemMark;

    unsafe fn mark(&mut self) -> Self::Mark {
        unsafe { self.access.scratch_memory().mark() }
    }
    unsafe fn pop(&mut self, mark: Self::Mark) {
        unsafe { self.access.scratch_memory().pop(&mark); }
    }
    unsafe fn has_hash(&mut self) -> bool {
        !unsafe { (*self.level).hash }.is_null()
    }
    unsafe fn level_info(&mut self) -> RuntimeResult<*mut AActor> {
        let count = unsafe { crate::level_owner::actor_count(self.level)? };
        if count <= 0 {
            return Err(RuntimeError::new("CheckEncroachment Level has no level-info Actor"));
        }
        Ok(unsafe { crate::level_owner::actor_at(self.level, 0) })
    }
    unsafe fn hash_check(
        &mut self,
        actor: *mut AActor,
        location: FVector,
        rotation: FRotator,
    ) -> RuntimeResult<*mut FCheckResult> {
        unsafe { crate::level_owner::hash_encroachment_check(self.access, self.level, actor, location, rotation) }
    }
    unsafe fn is_moving_brush(&mut self, actor: *mut AActor) -> RuntimeResult<bool> {
        if unsafe { (*actor).brush.is_null() } {
            return Ok(false);
        }
        Ok(unsafe { self.is_a(actor, crate::actor::class::ABRUSH)? } && !unsafe { (*actor).b_static() })
    }
    unsafe fn is_blocked_by(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<bool> {
        unsafe {
            crate::actor::is_blocked_by_with(actor, other, |value, kind| self.is_a(value.cast(), kind))
        }
    }
    unsafe fn move_smooth(&mut self, actor: *mut AActor, delta: FVector) -> RuntimeResult<()> {
        unsafe { crate::actor_physics_owner::move_smooth(self.access, actor, delta)?; }
        Ok(())
    }
    unsafe fn encroaching_on(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<bool> {
        unsafe { self.event_with_actor_result(actor, "EncroachingOn", other) }
    }
    unsafe fn is_overlapping(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<bool> {
        unsafe {
            crate::actor::is_overlapping_with(actor, other, |value| {
                if (*value).brush.is_null() {
                    Ok(false)
                } else {
                    self.is_a(value, crate::actor::class::ABRUSH)
                }
            })
        }
        .map(|overlaps| overlaps != 0)
    }
    unsafe fn end_touch(&mut self, actor: *mut AActor, other: *mut AActor, no_notify_self: bool) -> RuntimeResult<()> {
        unsafe { CanonicalCheckEncroachmentOps { access: self.access, level: self.level }.end_touch(actor, other, no_notify_self) }
    }
    unsafe fn begin_touch(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        unsafe { CanonicalCheckEncroachmentOps::begin_touch(self, actor, other) }
    }
    unsafe fn encroached_by(&mut self, actor: *mut AActor, other: *mut AActor) -> RuntimeResult<()> {
        unsafe { self.event_with_actor(actor, "EncroachedBy", other) }
    }
}

/// Runs the shared `ULevel::CheckEncroachment` policy on Core-owned records.
///
/// # Safety
/// Level and Actor are live canonical records; any MoveSmooth reentry remains synchronous.
pub unsafe fn check_encroachment(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    actor: *mut AActor,
    test_location: FVector,
    test_rotation: FRotator,
    touch_notify: bool,
) -> RuntimeResult<bool> {
    if level.is_null() || actor.is_null() {
        return Err(RuntimeError::new("CheckEncroachment received a null Level or Actor"));
    }
    let mut ops = CanonicalCheckEncroachmentOps { access, level };
    unsafe {
        levact::check_encroachment_policy(
            &mut ops,
            actor,
            test_location,
            test_rotation,
            touch_notify,
        )
    }
}
