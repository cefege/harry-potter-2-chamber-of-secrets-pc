//! Direct canonical Core ownership of `ULevel::SpawnActor` operations.

use crate::level_spawn_actor::{self, Hp2SpawnActorContext, Op, SpawnActorOps};
use hp2_abi::engine::generated::{
    AActor, ALevelInfo, APawn, FActorLink, FName, FPointRegion, FRotator, TArrayRaw,
    UClass, ULevel, UObject, UProperty,
};
use hp2_abi::FVector;
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{ffi::c_void, mem, ptr, ptr::NonNull};

const ROLE_AUTHORITY: u8 = 4;
const PHYS_NONE: u8 = 0;
const PHYS_ROTATING: u8 = 5;
const NM_CLIENT: u8 = 3;
const RF_TRANSACTIONAL: u32 = 1;
const CLASS_ABSTRACT: u32 = 1;

struct CanonicalSpawnActorOps<'a> {
    access: CoreAccess<'a>,
}
impl CanonicalSpawnActorOps<'_> {
    fn warn(&self, message: &str) -> RuntimeResult<()> {
        let mut encoded = [0u32; 128];
        let bytes = message.as_bytes();
        if bytes.len() > encoded.len() {
            return Err(RuntimeError::new("SpawnActor warning exceeds the fixed message buffer"));
        }
        for (output, input) in encoded.iter_mut().zip(bytes) {
            *output = u32::from(*input);
        }
        let category = self.access.intern_str("Warning", FindName::Add)?;
        self.access.emit_log(category, &encoded[..bytes.len()])
    }
    fn warn_class(&self, prefix: &str, class: *mut UClass, suffix: &str) -> RuntimeResult<()> {
        let name_index = unsafe { i32::from_ne_bytes((*class.cast::<UObject>()).name) };
        let name = self.access.name_text(name_index)?;
        let mut message = [0u32; 128];
        let mut length = 0;
        for byte in prefix.as_bytes() {
            message[length] = u32::from(*byte);
            length += 1;
        }
        if length + name.len() + suffix.len() > message.len() {
            return Err(RuntimeError::new("SpawnActor warning exceeds the fixed message buffer"));
        }
        message[length..length + name.len()].copy_from_slice(&name);
        length += name.len();
        for byte in suffix.as_bytes() {
            message[length] = u32::from(*byte);
            length += 1;
        }
        let category = self.access.intern_str("Warning", FindName::Add)?;
        self.access.emit_log(category, &message[..length])
    }

    fn instrumentation(&self, event: crate::core_owner::SpawnInstrumentationEvent) -> RuntimeResult<()> {
        notify_spawn_instrumentation(self.access, event)
    }

    fn observe_lifecycle(&self, actor: *mut AActor, event: crate::core_owner::ActorLifecycleEvent) -> RuntimeResult<()> {
        notify_actor_lifecycle(self.access, actor, event)
    }

    fn observe_published(&self, actor: *mut AActor) -> RuntimeResult<()> {
        let state = self.access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("SpawnActor has no canonical Engine memory owner"))?;
        if let Some(observer) = state.actor_spawned_observer.get() {
            let actor = NonNull::new(actor).ok_or_else(|| RuntimeError::new("OnActorSpawned observer received a null Actor"))?;
            observer(self.access, actor)?;
        }
        Ok(())
    }
}

pub(crate) fn notify_spawn_instrumentation(
    access: CoreAccess<'_>,
    event: crate::core_owner::SpawnInstrumentationEvent,
) -> RuntimeResult<()> {
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("SpawnActor has no canonical Engine memory owner"))?;
    let provider = state.spawn_instrumentation.borrow().clone();
    if let Some(provider) = provider {
        provider.on_spawn_instrumentation(access, event)?;
    }
    Ok(())
}

pub(crate) fn notify_actor_lifecycle(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    event: crate::core_owner::ActorLifecycleEvent,
) -> RuntimeResult<()> {
    let state = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("SpawnActor has no canonical Engine memory owner"))?;
    if let Some(observer) = state.actor_lifecycle_observer.get() {
        let actor = NonNull::new(actor).ok_or_else(|| RuntimeError::new("SpawnActor lifecycle observer received a null Actor"))?;
        observer(access, actor, event)?;
    }
    Ok(())
}
impl CanonicalSpawnActorOps<'_> {
    unsafe fn actor(&self, value: usize) -> RuntimeResult<*mut AActor> {
        let actor = value as *mut AActor;
        if actor.is_null() { return Err(RuntimeError::new("SpawnActor received a null Actor")); }
        Ok(actor)
    }

    unsafe fn level_info(&self, level: *mut ULevel) -> RuntimeResult<*mut ALevelInfo> {
        let count = unsafe { crate::level_owner::actor_count(level)? };
        if count <= 0 { return Err(RuntimeError::new("SpawnActor Level has no level-info Actor")); }
        let info = unsafe { crate::level_owner::actor_at(level, 0) }.cast::<ALevelInfo>();
        if info.is_null() { return Err(RuntimeError::new("SpawnActor Level actor zero is null")); }
        Ok(info)
    }

    unsafe fn relation_event(
        &self,
        event: crate::actor::ActorRelationEvent,
        receiver: *mut c_void,
        related: *mut c_void,
    ) -> RuntimeResult<()> {
        if receiver.is_null() { return Ok(()); }
        let name = match event {
            crate::actor::ActorRelationEvent::LostChild => "LostChild",
            crate::actor::ActorRelationEvent::GainedChild => "GainedChild",
            crate::actor::ActorRelationEvent::Detach => "Detach",
            crate::actor::ActorRelationEvent::Attach => "Attach",
            crate::actor::ActorRelationEvent::BaseChange => "BaseChange",
            crate::actor::ActorRelationEvent::UnTouch => "UnTouch",
        };
        if event == crate::actor::ActorRelationEvent::BaseChange {
            unsafe { self.access.execute_named_event(receiver.cast(), name)?; }
            return Ok(());
        }
        let argument = related;
        let function_name = self.access.intern_str(name, FindName::Find)?;
        let function = unsafe { self.access.find_object_function_by_name(receiver.cast(), function_name, false)? };
        if !function.is_null() {
            unsafe { self.access.execute_event(receiver.cast(), function, (&argument as *const *mut c_void).cast_mut().cast(), ptr::null_mut())?; }
        }
        Ok(())
    }

    unsafe fn spawn_notify_field(
        &self,
        notify: *mut UObject,
        field: &str,
    ) -> RuntimeResult<*mut c_void> {
        let name = self.access.intern_str(field, FindName::Find)?;
        let property_class = self.access.native_class("ObjectProperty")?;
        let class = unsafe { ptr::addr_of!((*notify).class).read().cast::<UClass>() };
        let property = unsafe { self.access.find_field(class.cast(), name, property_class) }.cast::<UProperty>();
        if property.is_null() { return Err(RuntimeError::new("SpawnNotify is missing a required reflected field")); }
        let address = unsafe { self.access.property_address(notify, property)? };
        Ok(unsafe { address.cast::<*mut c_void>().read_unaligned() })
    }

    unsafe fn notify(&self, notify: *mut UObject, actor: *mut AActor) -> RuntimeResult<*mut AActor> {
        let name = self.access.intern_str("SpawnNotification", FindName::Find)?;
        let function = unsafe { self.access.find_object_function_by_name(notify, name, false)? };
        if function.is_null() { return Err(RuntimeError::new("SpawnNotify has no SpawnNotification event")); }
        #[repr(C)]
        struct Params { actor: *mut AActor, result: *mut AActor }
        if unsafe { (*function).parms_size } as usize != std::mem::size_of::<Params>()
            || unsafe { (*function).num_parms } != 2
            || unsafe { (*function).return_value_offset } as usize != std::mem::offset_of!(Params, result)
        {
            return Err(RuntimeError::new("SpawnNotification parameter layout does not match metadata"));
        }
        let mut params = Params { actor, result: ptr::null_mut() };
        unsafe {
            self.access.execute_event(notify, function, (&mut params as *mut Params).cast(), ptr::null_mut())?;
        }
        if params.result.is_null() { return Err(RuntimeError::new("SpawnNotification returned a null Actor")); }
        Ok(params.result)
    }

    unsafe fn set_owner(&self, actor: *mut AActor, owner: *mut c_void) -> RuntimeResult<()> {
        let mut ops = self;
        unsafe { crate::actor::set_owner_with_ops(&mut ops, actor, owner) }
    }
}

impl crate::actor::ActorRelationOps for &CanonicalSpawnActorOps<'_> {
    type Error = RuntimeError;
    unsafe fn event(
        &mut self,
        event: crate::actor::ActorRelationEvent,
        receiver: *mut c_void,
        related: *mut c_void,
    ) -> RuntimeResult<()> {
        unsafe { self.relation_event(event, receiver, related) }
    }
}

impl SpawnActorOps for CanonicalSpawnActorOps<'_> {
    type Error = RuntimeError;

    unsafe fn call(&self, context: *mut c_void, op: Op, a: usize, b: usize) -> RuntimeResult<usize> {
        let context = context.cast::<Hp2SpawnActorContext>();
        if context.is_null() { return Err(RuntimeError::new("Null SpawnActor context")); }
        let level = unsafe { (*context).level.cast::<ULevel>() };
        if level.is_null() { return Err(RuntimeError::new("Null Level in SpawnActor")); }
        let actor = a as *mut AActor;
        match op {
            Op::TransitionRequest => {
                self.instrumentation(crate::core_owner::SpawnInstrumentationEvent::TransitionRequest {
                    class: unsafe { (*context).class.cast() },
                    name: unsafe { (*context).in_name },
                    owner: unsafe { (*context).owner.cast() },
                })?;
                Ok(0)
            }
            Op::CreatureRequest => {
                self.instrumentation(crate::core_owner::SpawnInstrumentationEvent::CreatureRequest {
                    class: unsafe { (*context).class.cast() },
                    owner: unsafe { (*context).owner.cast() },
                })?;
                Ok(0)
            }
            Op::TransitionResult => {
                self.instrumentation(crate::core_owner::SpawnInstrumentationEvent::TransitionResult {
                    owner: unsafe { (*context).owner.cast() },
                    actor,
                })?;
                Ok(0)
            }
            Op::CreatureResult => {
                self.instrumentation(crate::core_owner::SpawnInstrumentationEvent::CreatureResult {
                    owner: unsafe { (*context).owner.cast() },
                    actor,
                })?;
                Ok(0)
            }
            Op::TransitionPublished => {
                self.instrumentation(crate::core_owner::SpawnInstrumentationEvent::TransitionPublished {
                    owner: unsafe { (*context).owner.cast() },
                    actor,
                })?;
                Ok(0)
            }
            Op::CreaturePublished => {
                self.instrumentation(crate::core_owner::SpawnInstrumentationEvent::CreaturePublished {
                    owner: unsafe { (*context).owner.cast() },
                    actor,
                })?;
                Ok(0)
            }
            Op::PreBeginBefore | Op::PreBeginAfter => {
                let event = if matches!(op, Op::PreBeginBefore) {
                    crate::core_owner::SpawnInstrumentationEvent::PreBeginBefore { actor }
                } else {
                    crate::core_owner::SpawnInstrumentationEvent::PreBeginAfter { actor }
                };
                self.instrumentation(event)?;
                Ok(0)
            }
            Op::Observer => {
                let state = self.access.owner_context::<crate::core_owner::EngineState>()
                    .ok_or_else(|| RuntimeError::new("SpawnActor has no canonical Engine memory owner"))?;
                Ok(usize::from(state.actor_lifecycle_observer.get().is_some()
                    || state.actor_spawned_observer.get().is_some()))
            }
            Op::ObserveSpawned | Op::ObservePreBegin | Op::ObserveBegin
            | Op::ObservePostBegin | Op::ObserveInitialState => {
                use crate::core_owner::ActorLifecycleEvent;
                let event = match op {
                    Op::ObserveSpawned => ActorLifecycleEvent::Spawned,
                    Op::ObservePreBegin => ActorLifecycleEvent::PreBeginPlay,
                    Op::ObserveBegin => ActorLifecycleEvent::BeginPlay,
                    Op::ObservePostBegin => ActorLifecycleEvent::PostBeginPlay,
                    Op::ObserveInitialState => ActorLifecycleEvent::SetInitialState,
                    _ => unreachable!(),
                };
                self.observe_lifecycle(actor, event)?;
                Ok(0)
            }
            Op::ObservePublished => {
                self.observe_published(actor)?;
                Ok(0)
            }
            // The outer API attaches RuntimeError context after the policy unwinds.
            Op::Unwind => Ok(0),
            Op::Class => Ok(unsafe { (*context).class as usize }),
            Op::ClassAbstract => {
                let class = unsafe { (*context).class.cast::<UClass>() };
                Ok(usize::from(!class.is_null() && unsafe { (*class).class_flags & CLASS_ABSTRACT != 0 }))
            }
            Op::ClassIsActor => {
                let class = unsafe { (*context).class.cast::<UClass>() };
                let actor_class = self.access.native_class("Engine.Actor")?;
                Ok(usize::from(!class.is_null() && unsafe { hp2_core::object_model::is_child_of(class.cast(), actor_class.cast()) }))
            }
            Op::Editor => Ok(usize::from(self.access.process_mode().is_editor)),
            Op::DefaultActor => {
                let class = unsafe { (*context).class.cast::<UClass>() };
                if class.is_null() { return Ok(0); }
                Ok(unsafe { self.access.default_object(class)? as usize })
            }
            Op::Static | Op::NoDelete => {
                let actor = unsafe { self.actor(a)? };
                Ok(usize::from(if matches!(op, Op::Static) { unsafe { (*actor).b_static() } } else { unsafe { (*actor).b_no_delete() } }))
            }
            Op::WarnMissingClass => { self.warn("SpawnActor failed because no class was specified")?; Ok(0) }
            Op::WarnAbstract => { self.warn_class("SpawnActor failed because class ", unsafe { (*context).class.cast() }, " is abstract")?; Ok(0) }
            Op::WarnNotActor => { self.warn_class("SpawnActor failed because ", unsafe { (*context).class.cast() }, " is not an actor class")?; Ok(0) }
            Op::WarnStatic => { self.warn_class("SpawnActor failed because class ", unsafe { (*context).class.cast() }, " has bStatic or bNoDelete")?; Ok(0) }
            Op::WarnFit => { self.warn_class("SpawnActor failed because ", unsafe { (*context).class.cast() }, " will not fit in location")?; Ok(0) }
            Op::Template => Ok(unsafe { (*context).template as usize }),
            Op::CheckTemplate => { if (a as *mut AActor).is_null() { return Err(RuntimeError::new("SpawnActor template is null")); } Ok(0) }
            Op::CollideWorld | Op::CollideWhenPlacing => {
                let actor = unsafe { self.actor(a)? };
                Ok(usize::from(if matches!(op, Op::CollideWorld) { unsafe { (*actor).b_collide_world() } } else { unsafe { (*actor).b_collide_when_placing() } }))
            }
            Op::NetIsClient => Ok(usize::from(unsafe { (*self.level_info(level)?).net_mode == NM_CLIENT })),
            Op::NoCollisionFail => Ok(usize::from(unsafe { (*context).no_collision_fail != 0 })),
            Op::FindSpot => {
                let template = unsafe { self.actor(a)? };
                let location = unsafe { (*context).location.cast_mut() };
                let extent = unsafe { crate::collision_owner::actor_cylinder_extent(self.access, template)? };
                Ok(usize::from(unsafe { crate::level_zone_owner::find_spot(self.access, level, extent, location, false, true)? }))
            }
            Op::AddSlot => {
                let trans = unsafe { ptr::addr_of_mut!((*level).base.actors) };
                let array = trans.cast::<TArrayRaw>();
                let slots = Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*array).data) }, num: unsafe { ptr::addr_of_mut!((*array).num) }, max: unsafe { ptr::addr_of_mut!((*array).max) } };
                let index = unsafe { array_storage::add(slots, 1, mem::size_of::<*mut AActor>() as i32, ptr::null(), array_storage::system_allocator()) }
                    .map_err(|_| RuntimeError::new("Unable to grow Level Actors array"))?;
                unsafe { self.access.add_trans_array_items(trans, index, 1)?; }
                Ok(index as usize)
            }
            Op::Construct => {
                let class = unsafe { (*context).class.cast::<UClass>() };
                let outer = unsafe { (*level).base.base.outer.cast::<UObject>() };
                let template = a as *mut UObject;
                Ok(unsafe { self.access.construct_object(class, outer, (*context).in_name.index, 0, template)? as usize })
            }
            Op::PublishSlot => {
                let index = b as i32;
                let trans = unsafe { ptr::addr_of_mut!((*level).base.actors) };
                if index < 0 || index >= unsafe { (*trans).array_num } { return Err(RuntimeError::new("Invalid Level Actors slot")); }
                unsafe { (*trans).data.cast::<*mut AActor>().add(index as usize).write_unaligned(actor); }
                Ok(0)
            }
            Op::Transactional => { let actor = unsafe { self.actor(a)? }; unsafe { (*actor.cast::<UObject>()).object_flags |= RF_TRANSACTIONAL; } Ok(0) }
            Op::SetTag => {
                let class = unsafe { (*context).class.cast::<UObject>() };
                if class.is_null() { return Err(RuntimeError::new("SpawnActor class is null")); }
                unsafe { ptr::copy_nonoverlapping((*class).name.as_ptr(), (*self.actor(a)?).tag.as_mut_ptr(), 4); }
                Ok(0)
            }
            Op::SetRegion => {
                let actor = unsafe { self.actor(a)? };
                let mut region: FPointRegion = unsafe { mem::zeroed() };
                region.zone = unsafe { self.level_info(level)? }.cast();
                region.i_leaf = -1;
                unsafe { (*actor).region = region; }
                Ok(0)
            }
            Op::SetLevel => { unsafe { (*self.actor(a)?).level = self.level_info(level)?.cast(); } Ok(0) }
            Op::Ticked => Ok(usize::from(unsafe { (*level).ticked != 0 })),
            Op::SetTicked => { unsafe { (*self.actor(a)?).set_b_ticked(b != 0); } Ok(0) }
            Op::SetXLevel => { unsafe { (*self.actor(a)?).x_level = level.cast(); } Ok(0) }
            Op::CheckAuthority => { if unsafe { (*self.actor(a)?).role } != ROLE_AUTHORITY { return Err(RuntimeError::new("Spawned Actor does not have authority role")); } Ok(0) }
            Op::RemoteOwned => Ok(usize::from(unsafe { (*context).remote_owned != 0 })),
            Op::SwapRoles => { let actor = unsafe { self.actor(a)? }; unsafe { ptr::swap(ptr::addr_of_mut!((*actor).role), ptr::addr_of_mut!((*actor).remote_role)); } Ok(0) }
            Op::Brush => Ok(unsafe { (*self.actor(a)?).brush as usize }),
            Op::ClearBrush => { unsafe { (*self.actor(a)?).brush = ptr::null_mut(); } Ok(0) }
            Op::SetLocation => { unsafe { (*self.actor(a)?).location = (*context).location.read(); } Ok(0) }
            Op::SetOldLocation => { unsafe { (*self.actor(a)?).old_location = (*context).location.read(); } Ok(0) }
            Op::SetRotation => { unsafe { (*self.actor(a)?).rotation = (*context).rotation.read(); } Ok(0) }
            Op::CollideActors => Ok(usize::from(unsafe { (*self.actor(a)?).b_collide_actors() })),
            Op::Hash => Ok(unsafe { (*level).hash as usize }),
            Op::HashAdd => { if !unsafe { (*level).hash }.is_null() { unsafe { crate::level_owner::hash_add(self.access, level, self.actor(a)?)?; } } Ok(0) }
            Op::HashRemove => { if !unsafe { (*level).hash }.is_null() { unsafe { crate::level_owner::hash_remove(self.access, level, self.actor(a)?, false)?; } } Ok(0) }
            Op::IsPawn => { let class = self.access.native_class("Engine.Pawn")?; Ok(usize::from(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })) }
            Op::SetPawnRegions => { let actor = unsafe { self.actor(a)? }; let region = unsafe { (*actor).region }; let pawn = actor.cast::<APawn>(); unsafe { (*pawn).foot_region = region; (*pawn).head_region = region; } Ok(0) }
            Op::SetOwner => { unsafe { self.set_owner(self.actor(a)?, (*context).owner)?; } Ok(0) }
            Op::SetInstigator => { unsafe { (*self.actor(a)?).instigator = (*context).instigator; } Ok(0) }
            Op::InitExecution => { unsafe { self.access.initialize_execution(actor.cast())?; } Ok(0) }
            Op::Spawned => { unsafe { crate::actor_owner::spawned(self.access, self.actor(a)?)?; } Ok(0) }
            Op::EventSpawned => { unsafe { self.access.execute_named_event(actor.cast(), "Spawned")?; } Ok(0) }
            Op::PreBeginPlay => { unsafe { self.access.execute_named_event(actor.cast(), "PreBeginPlay")?; } Ok(0) }
            Op::BeginPlay => { unsafe { self.access.execute_named_event(actor.cast(), "BeginPlay")?; } Ok(0) }
            Op::DeleteMe => Ok(usize::from(unsafe { (*self.actor(a)?).b_delete_me() })),
            Op::SetZone => { unsafe { crate::level_zone_owner::set_actor_zone(self.access, level, self.actor(a)?, b != 0, true)?; } Ok(0) }
            Op::PostBeginPlay => { unsafe { self.access.execute_named_event(actor.cast(), "PostBeginPlay")?; } Ok(0) }
            Op::Encroachment => {
                let actor = unsafe { self.actor(a)? };
                let location = unsafe { (*actor).location };
                let rotation = unsafe { (*actor).rotation };
                let encroaches = unsafe {
                    crate::level_encroachment_owner::check_encroachment(
                        self.access,
                        level,
                        actor,
                        location,
                        rotation,
                        false,
                    )?
                };
                Ok(usize::from(encroaches))
            }
            Op::Destroy => { unsafe { crate::level_lifecycle_owner::destroy_actor(self.access, level, self.actor(a)?, false)?; } Ok(0) }
            Op::InitialState => { unsafe { self.access.execute_named_event(actor.cast(), "SetInitialState")?; } Ok(0) }
            Op::Base => Ok(unsafe { (*self.actor(a)?).base as usize }),
            Op::IsDecoration | Op::IsInventory => {
                let class = self.access.native_class(if matches!(op, Op::IsDecoration) { "Engine.Decoration" } else { "Engine.Inventory" })?;
                Ok(usize::from(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) }))
            }
            Op::PhysicsNone | Op::PhysicsRotating => { let physics = unsafe { (*self.actor(a)?).physics }; Ok(usize::from(physics == if matches!(op, Op::PhysicsNone) { PHYS_NONE } else { PHYS_ROTATING })) }
            Op::FindBase => { unsafe { crate::actor_physics_owner::find_base(self.access, self.actor(a)?)?; } Ok(0) }
            Op::InTick => Ok(usize::from(unsafe { (*level).in_tick != 0 })),
            Op::LinkNewlySpawned => {
                let state = self.access.owner_context::<crate::core_owner::EngineState>().ok_or_else(|| RuntimeError::new("SpawnActor has no canonical Engine memory owner"))?;
                let link = unsafe { state.memory.push_bytes(mem::size_of::<FActorLink>() as i32, mem::align_of::<FActorLink>() as i32)? }.cast::<FActorLink>();
                unsafe { link.write(FActorLink { actor: self.actor(a)?.cast(), next: (*level).newly_spawned }); (*level).newly_spawned = link.cast(); }
                Ok(0)
            }
            Op::FirstNotify => {
                let info = unsafe { self.level_info(level)? };
                let field_name = self.access.intern_str("SpawnNotify", FindName::Find)?;
                let property_class = self.access.native_class("ObjectProperty")?;
                let structure = unsafe { ptr::addr_of!((*info.cast::<UObject>()).class).read().cast() };
                let field = unsafe { self.access.find_field(structure, field_name, property_class) };
                if field.is_null() { return Ok(0); }
                let address = unsafe { self.access.property_address(info.cast(), field.cast())? };
                Ok(unsafe { address.cast::<*mut c_void>().read_unaligned() as usize })
            }
            Op::NotifyClass => Ok(unsafe { self.spawn_notify_field((a as *mut UObject), "ActorClass")? as usize }),
            Op::IsA => { let class = b as *mut UClass; Ok(usize::from(unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) })) }
            Op::Notify => Ok(unsafe { self.notify(a as *mut UObject, b as *mut AActor)? as usize }),
            Op::NextNotify => Ok(unsafe { self.spawn_notify_field((a as *mut UObject), "Next")? as usize }),
        }
    }
}

/// Runs the source-ordered SpawnActor policy on canonical Core-owned objects.
///
/// # Errors
/// Returns Core ownership, reflection, lifecycle, collision, memory, or VM errors.
///
/// # Safety
/// `level`, `class`, and non-null actor inputs are live canonical records in `access`;
/// callbacks remain synchronous and do not unwind.
#[allow(clippy::too_many_arguments)]
pub unsafe fn spawn_actor(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    class: *mut UClass,
    in_name: FName,
    owner: *mut AActor,
    instigator: *mut APawn,
    location: FVector,
    rotation: FRotator,
    template: *mut AActor,
    no_collision_fail: bool,
    remote_owned: bool,
) -> RuntimeResult<*mut AActor> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in SpawnActor")); }
    let context = Hp2SpawnActorContext {
        level: level.cast(), class: class.cast(), in_name, owner: owner.cast(), instigator: instigator.cast(),
        location: &location, rotation: &rotation, template: template.cast(),
        no_collision_fail: i32::from(no_collision_fail), remote_owned: i32::from(remote_owned),
    };
    let ops = CanonicalSpawnActorOps { access };
    // SAFETY: all source context inputs are live canonical records; call operations are synchronous.
    let outcome = unsafe {
        level_spawn_actor::spawn_actor_with_ops(
            &ops,
            (&context as *const Hp2SpawnActorContext).cast_mut().cast(),
        )
    };
    let actor = outcome.map_err(|error| {
        let class_name = if class.is_null() {
            "null class".to_owned()
        } else {
            let index = unsafe { i32::from_ne_bytes((*class.cast::<UObject>()).name) };
            match access.name_text(index) {
                Ok(units) => units.iter().filter_map(|unit| char::from_u32(*unit)).collect::<String>(),
                Err(_) => "unknown class".to_owned(),
            }
        };
        error.with_context(&format!("ULevel::SpawnActor({class_name})"))
    })?;
    Ok(actor as *mut AActor)
}
