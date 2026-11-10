//! Direct canonical ownership of `ULevel::DestroyActor` lifecycle transitions.

use hp2_abi::engine::generated::{
    AActor, APlayerPawn, FStateFrame, UChannel, UClass, UField, UEngine, ULevel, UObject,
    UObjectProperty, UProperty, UStruct, UStructProperty, UNetConnection, UNetDriver,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::ptr;
const USOCK_CLOSED: i32 = 1;

struct CanonicalDestroyActorRules<'a> {
    access: CoreAccess<'a>,
    channel: *mut UChannel,
}

impl crate::level_lifecycle::DestroyActorRuleOps for CanonicalDestroyActorRules<'_> {
    type Error = RuntimeError;

    unsafe fn is_editor(&mut self) -> Result<bool, RuntimeError> {
        Ok(self.access.process_mode().is_editor)
    }
    unsafe fn is_static(&mut self, actor: *mut std::ffi::c_void) -> Result<bool, RuntimeError> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_static() })
    }
    unsafe fn is_no_delete(&mut self, actor: *mut std::ffi::c_void) -> Result<bool, RuntimeError> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_no_delete() })
    }
    unsafe fn is_delete_me(&mut self, actor: *mut std::ffi::c_void) -> Result<bool, RuntimeError> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_delete_me() })
    }
    unsafe fn role(&mut self, actor: *mut std::ffi::c_void) -> Result<u8, RuntimeError> {
        Ok(unsafe { (*actor.cast::<AActor>()).role })
    }
    unsafe fn is_net_temporary(&mut self, actor: *mut std::ffi::c_void) -> Result<bool, RuntimeError> {
        Ok(unsafe { (*actor.cast::<AActor>()).b_net_temporary() })
    }
    unsafe fn connected_player_channel(
        &mut self, actor: *mut std::ffi::c_void,
    ) -> Result<bool, RuntimeError> {
        self.channel = ptr::null_mut();
        let pawn_class = self.access.native_class("Engine.PlayerPawn")?;
        let actor = actor.cast::<UObject>();
        if !unsafe { hp2_core::object_model::is_a(actor.cast(), pawn_class.cast()) } {
            return Ok(false);
        }
        let player = unsafe { (*actor.cast::<APlayerPawn>()).player.cast::<UObject>() };
        if player.is_null() {
            return Ok(false);
        }
        let connection_class = self.access.native_class("Engine.NetConnection")?;
        if !unsafe { hp2_core::object_model::is_a(player.cast(), connection_class.cast()) } {
            return Ok(false);
        }
        let connection = player.cast::<UNetConnection>();
        self.channel = unsafe { (*connection).channels[0].cast() };
        Ok(!self.channel.is_null() && unsafe { (*connection).state } != USOCK_CLOSED)
    }
    unsafe fn close_player_channel(
        &mut self, _actor: *mut std::ffi::c_void,
    ) -> Result<(), RuntimeError> {
        if !self.channel.is_null() {
            unsafe { crate::channel_owner::close(self.access, self.channel)?; }
        }
        Ok(())
    }
}

/// Runs source-ordered actor destruction on canonical Core-owned records.
///
/// Actor callbacks dispatch through the canonical VM; no UObject virtual dispatch,
/// native service table, or C++ object callback is used.
///
/// # Errors
/// Returns a canonical ownership, reflection, collision, transaction, network, or VM error.
///
/// # Safety
/// `level` and `actor` are live canonical records in `access`; callback-reachable
/// actors, drivers, channels, and arrays remain live for synchronous execution.
pub unsafe fn destroy_actor(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    this_actor: *mut AActor,
    net_force: bool,
) -> RuntimeResult<bool> {
    if level.is_null() || this_actor.is_null() {
        return Err(RuntimeError::new("Null Level or Actor in DestroyActor"));
    }
    let actor_object = this_actor.cast::<UObject>();
    let object_index = unsafe { (*actor_object).index };
    if object_index < 0 || access.object_at(object_index) != actor_object {
        return Err(RuntimeError::new("DestroyActor received a noncanonical Actor"));
    }
    let mut ops = CanonicalDestroyActorOps { access, level, actor: this_actor, net_force, actor_index: -1 };
    unsafe { crate::level_lifecycle::destroy_actor_with_ops(&mut ops) }
}

struct CanonicalDestroyActorOps<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
    actor: *mut AActor,
    net_force: bool,
    actor_index: i32,
}

impl crate::level_lifecycle::DestroyActorOps for CanonicalDestroyActorOps<'_> {
    type Error = RuntimeError;

    unsafe fn early_result(&mut self) -> Result<Option<bool>, RuntimeError> {
        let mut rules = CanonicalDestroyActorRules {
            access: self.access,
            channel: ptr::null_mut(),
        };
        unsafe {
            crate::level_lifecycle::destroy_actor_early_rules(
                &mut rules,
                self.actor.cast(),
                self.net_force,
            )
        }
    }

    unsafe fn stage(
        &mut self,
        stage: crate::level_lifecycle::DestroyActorStage,
    ) -> Result<Option<bool>, RuntimeError> {
        let actor = self.actor;
        let object = actor.cast::<UObject>();
        let access = self.access;
        let level = self.level;
        match stage {
            crate::level_lifecycle::DestroyActorStage::ModifyActor => {
                let actor_count = unsafe { crate::level_owner::actor_count(level)? };
                self.actor_index = 0;
                while self.actor_index < actor_count
                    && unsafe { crate::level_owner::actor_at(level, self.actor_index) } != actor
                {
                    self.actor_index += 1;
                }
                if self.actor_index == actor_count {
                    // The original appErrorf named no actor; the full name and
                    // level state are embedded so the failing Destroy() is
                    // identifiable.
                    let name = unsafe { access.object_full_name(object) }.unwrap_or_default();
                    let mut message: Vec<u32> =
                        "Actor not found in Level actor array: ".chars().map(u32::from).collect();
                    message.extend(name.into_iter().take_while(|&unit| unit != 0));
                    let detail = format!(" (level={:p} actors={actor_count})", level);
                    message.extend(detail.chars().map(u32::from));
                    return Err(RuntimeError::from_utf32(message));
                }
                let actors = unsafe { ptr::addr_of_mut!((*level).base.actors) };
                unsafe {
                    access.modify_trans_array_item(actors, self.actor_index)?;
                    access.modify_object(object)?;
                }
            }
            crate::level_lifecycle::DestroyActorStage::EndState => {
                let state_frame = unsafe {
                    ptr::addr_of!((*object).state_frame).read().cast::<FStateFrame>()
                };
                if !state_frame.is_null() && unsafe { !(*state_frame).state_node.is_null() } {
                    let end_state = access.intern_str(
                        "EndState",
                        hp2_core::name_owner::FindName::Find,
                    )?;
                    if unsafe { access.is_probing(object, end_state)? } {
                        unsafe { access.execute_state_event(object, "EndState")?; }
                        if unsafe { (*actor).b_delete_me() } {
                            return Ok(Some(true));
                        }
                    }
                }
            }
            crate::level_lifecycle::DestroyActorStage::Debase => {
                if !unsafe { (*actor).base }.is_null() {
                    unsafe { set_base(access, actor, ptr::null_mut(), true)?; }
                    if unsafe { (*actor).b_delete_me() } {
                        return Ok(Some(true));
                    }
                }
                if unsafe { (*actor).standing_count } > 0 {
                    let mut index = 0;
                    while index < unsafe { crate::level_owner::actor_count(level)? } {
                        let other = unsafe { crate::level_owner::actor_at(level, index) };
                        if !other.is_null() && unsafe { (*other).base } == actor.cast() {
                            unsafe { set_base(access, other, ptr::null_mut(), true)?; }
                        }
                        index += 1;
                    }
                }
            }
            crate::level_lifecycle::DestroyActorStage::Unhash => {
                let hash = unsafe { (*level).hash };
                if !hash.is_null() {
                    if unsafe { (*actor).b_collide_actors() } {
                        unsafe { crate::level_owner::hash_remove(access, level, actor, true)?; }
                    }
                    unsafe { crate::level_owner::hash_check_actor_not_referenced(level, actor)?; }
                }
            }
            crate::level_lifecycle::DestroyActorStage::ProcessDestroyed => {
                unsafe { access.execute_named_event(object, "Destroyed")?; }
                if unsafe { (*actor).b_delete_me() } {
                    return Ok(Some(true));
                }
            }
            crate::level_lifecycle::DestroyActorStage::CleanupStandardRefs => {
                let mut index = 0;
                while index < unsafe { crate::level_owner::actor_count(level)? } {
                    let other = unsafe { crate::level_owner::actor_at(level, index) };
                    if !other.is_null() {
                        unsafe {
                            if (*other).tick_parent == actor.cast() {
                                (*other).tick_parent = ptr::null_mut();
                            }
                            if (*other).tick_parent2 == actor.cast() {
                                (*other).tick_parent2 = ptr::null_mut();
                            }
                            if (*other).owner == actor.cast() {
                                set_owner(access, other, ptr::null_mut())?;
                                if (*actor).b_delete_me() {
                                    return Ok(Some(true));
                                }
                            } else {
                                let touching = ptr::addr_of!((*other).touching).read_unaligned();
                                for touched in touching {
                                    if touched == actor.cast() {
                                        end_touch(access, actor, other, 1)?;
                                        if (*actor).b_delete_me() {
                                            return Ok(Some(true));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    index += 1;
                }
            }
            crate::level_lifecycle::DestroyActorStage::Disown => {
                let owner = unsafe { (*actor).owner.cast::<UObject>() };
                if !owner.is_null() {
                    unsafe { relation_event(access, owner, "LostChild", actor.cast())?; }
                    if unsafe { (*actor).b_delete_me() } {
                        return Ok(Some(true));
                    }
                }
            }
            crate::level_lifecycle::DestroyActorStage::NotifyDrivers => {
                let driver = unsafe { ptr::addr_of!((*level).base.net_driver).read_unaligned() };
                if !driver.is_null() {
                    unsafe {
                        crate::network_owner::notify_actor_destroyed(access, driver.cast(), actor)?;
                    }
                }
                let demo_driver =
                    unsafe { ptr::addr_of!((*level).base.demo_rec_driver).read_unaligned() };
                if !demo_driver.is_null()
                    && unsafe { (*demo_driver.cast::<UNetDriver>()).server_connection.is_null() }
                {
                    unsafe {
                        crate::network_owner::notify_actor_destroyed(
                            access,
                            demo_driver.cast(),
                            actor,
                        )?;
                    }
                }
            }
            crate::level_lifecycle::DestroyActorStage::UnlinkAndDestroy => {
                unsafe {
                    if crate::level_owner::actor_at(level, self.actor_index) != actor {
                        return Err(RuntimeError::new(
                            "Level actor slot changed before DestroyActor unlink",
                        ));
                    }
                    (*level).base.actors.data.cast::<*mut AActor>()
                        .add(self.actor_index as usize)
                        .write_unaligned(ptr::null_mut());
                    (*actor).set_b_delete_me(true);
                }
                if let Some(observer) = access
                    .owner_context::<crate::core_owner::EngineState>()
                    .and_then(|state| state.actor_lifecycle_observer.get())
                {
                    let actor = std::ptr::NonNull::new(actor)
                        .ok_or_else(|| RuntimeError::new("Null Actor in lifecycle observer"))?;
                    observer(access, actor, crate::core_owner::ActorLifecycleEvent::Destroying)?;
                }
                let engine = unsafe { ptr::addr_of!((*level).base.engine).read_unaligned() };
                if !engine.is_null() {
                    let engine_object = engine.cast::<UObject>();
                    let engine_index = unsafe { (*engine_object).index };
                    if engine_index < 0 || access.object_at(engine_index) != engine_object {
                        return Err(RuntimeError::new("Level Engine is not owned by this Core"));
                    }
                    let audio = unsafe { (*engine.cast::<UEngine>()).audio };
                    if !audio.is_null() {
                        let audio_object = audio.cast::<UObject>();
                        let audio_index = unsafe { (*audio_object).index };
                        if audio_index < 0 || access.object_at(audio_index) != audio_object {
                            return Err(RuntimeError::new("Engine.Audio is not owned by this Core"));
                        }
                        let state = access
                            .owner_context::<crate::core_owner::EngineState>()
                            .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
                        let binding = state.sound_audio.get().ok_or_else(|| {
                            RuntimeError::new("Canonical Engine.Audio has no audio owner binding")
                        })?;
                        if binding.receiver.as_ptr() != audio_object {
                            return Err(RuntimeError::new(
                                "Canonical Engine.Audio binding identity mismatch",
                            ));
                        }
                        let actor = std::ptr::NonNull::new(actor)
                            .ok_or_else(|| RuntimeError::new("Null Actor in audio destruction"))?;
                        (binding.note_destroy)(access, binding.receiver, actor)?;
                    }
                }
                unsafe { access.conditional_destroy(object)?; }
                if access.process_mode().is_editor {
                    unsafe { cleanup_destroyed_direct(access, level)?; }
                } else {
                    unsafe {
                        let first_deleted = ptr::addr_of!((*level).first_deleted).read_unaligned();
                        (*actor).deleted = first_deleted.cast();
                        ptr::addr_of_mut!((*level).first_deleted).write_unaligned(actor.cast());
                    }
                }
                return Ok(Some(true));
            }
        }
        Ok(None)
    }
}

unsafe fn cleanup_destroyed_direct(
    access: CoreAccess<'_>,
    level: *mut ULevel,
) -> RuntimeResult<()> {
    if unsafe { (*level).first_deleted }.is_null() {
        return Ok(());
    }
    let count = unsafe { crate::level_owner::actor_count(level)? };
    let property_class = access.native_class("Property")?;
    let object_property_class = access.native_class("ObjectProperty")?;
    let struct_property_class = access.native_class("StructProperty")?;
    let actor_class = access.native_class("Engine.Actor")?;
    let state = access
        .owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Engine owner context is unavailable"))?;
    let mut index = 0;
    while index < count {
        let active = unsafe { crate::level_owner::actor_at(level, index) };
        if !active.is_null() {
            let structure = unsafe { (*active.cast::<UObject>()).class.cast::<UStruct>() };
            // The reference slots of a class never change, so find them once per class
            // instead of walking every field and struct member of every actor each tick.
            let cached = state.cleanup_slots.borrow().get(&(structure as usize)).cloned();
            let slots = match cached {
                Some(slots) => slots,
                None => {
                    let mut offsets = Vec::new();
                    unsafe {
                        collect_reference_slots(
                            structure,
                            0,
                            property_class.cast::<UClass>(),
                            object_property_class.cast::<UClass>(),
                            struct_property_class.cast::<UClass>(),
                            &mut offsets,
                        );
                    }
                    let slots: std::rc::Rc<[usize]> = offsets.into();
                    state.cleanup_slots.borrow_mut().insert(structure as usize, slots.clone());
                    slots
                }
            };
            for &offset in slots.iter() {
                unsafe {
                    clear_deleted_actor_reference(
                        access,
                        active.cast::<u8>().add(offset).cast::<*mut UObject>(),
                        actor_class.cast::<UClass>(),
                        true,
                    )?;
                }
            }
        }
        index += 1;
    }
    Ok(())
}

struct CanonicalRelationOps<'a> {
    access: CoreAccess<'a>,
}

unsafe fn relation_event(
    access: CoreAccess<'_>,
    receiver: *mut UObject,
    event: &str,
    related: *mut std::ffi::c_void,
) -> RuntimeResult<()> {
    if event == "BaseChange" {
        unsafe { access.execute_named_event(receiver, event)?; }
        return Ok(());
    }
    if receiver.is_null() {
        return Err(RuntimeError::new("Null actor in relation event"));
    }
    let name = access.intern_str(event, hp2_core::name_owner::FindName::Find)?;
    let function = unsafe { access.find_object_function_by_name(receiver, name, false)? };
    if function.is_null() {
        return Ok(());
    }
    let mut related_object = related.cast::<UObject>();
    unsafe {
        access.execute_event(
            receiver,
            function,
            ptr::addr_of_mut!(related_object).cast(),
            ptr::null_mut(),
        )?;
    }
    Ok(())
}

impl crate::actor::ActorRelationOps for CanonicalRelationOps<'_> {
    type Error = RuntimeError;

    unsafe fn event(
        &mut self,
        event: crate::actor::ActorRelationEvent,
        receiver: *mut std::ffi::c_void,
        related: *mut std::ffi::c_void,
    ) -> RuntimeResult<()> {
        if event == crate::actor::ActorRelationEvent::UnTouch {
            crate::core_owner::observe_actor_movement(
                self.access,
                crate::core_owner::ActorMovementEvent::UnTouch,
                receiver.cast(),
                related.cast(),
            )?;
        }
        let name = match event {
            crate::actor::ActorRelationEvent::LostChild => "LostChild",
            crate::actor::ActorRelationEvent::GainedChild => "GainedChild",
            crate::actor::ActorRelationEvent::Detach => "Detach",
            crate::actor::ActorRelationEvent::Attach => "Attach",
            crate::actor::ActorRelationEvent::BaseChange => "BaseChange",
            crate::actor::ActorRelationEvent::UnTouch => "UnTouch",
        };
        unsafe { relation_event(self.access, receiver.cast(), name, related) }
    }
}

/// Rebases a canonical actor while preserving the source BaseChange notification flag.
///
/// # Safety
/// The actor and its new base chain must remain live in this Core graph.
pub unsafe fn set_base(
    access: CoreAccess<'_>, actor: *mut AActor, new_base: *mut AActor,
    notify_actor: bool,
) -> RuntimeResult<()> {
    let mut ops = CanonicalRelationOps { access };
    unsafe { crate::actor::set_base_with_ops(&mut ops, actor, new_base, notify_actor) }
}

pub(crate) unsafe fn set_owner(
    access: CoreAccess<'_>, actor: *mut AActor, new_owner: *mut AActor,
) -> RuntimeResult<()> {
    let mut ops = CanonicalRelationOps { access };
    unsafe { crate::actor::set_owner_with_ops(&mut ops, actor, new_owner.cast()) }
}

pub unsafe fn end_touch(
    access: CoreAccess<'_>, actor: *mut AActor, other: *mut AActor, no_notify_self: i32,
) -> RuntimeResult<()> {
    let mut ops = CanonicalRelationOps { access };
    unsafe { crate::actor::end_touch_with_ops(&mut ops, actor, other, no_notify_self) }
}

/// Records, in `cleanup_destroyed_refs`'s editor traversal order, the byte offset of
/// every object-reference slot reachable from `structure` (derived class first, nested
/// structs expanded per array element).
unsafe fn collect_reference_slots(
    structure: *mut UStruct,
    base: usize,
    property_class: *mut hp2_abi::engine::generated::UClass,
    object_property_class: *mut hp2_abi::engine::generated::UClass,
    struct_property_class: *mut hp2_abi::engine::generated::UClass,
    out: &mut Vec<usize>,
) {
    let mut current = structure;
    while !current.is_null() {
        let mut field = unsafe { (*current).children.cast::<UField>() };
        while !field.is_null() {
            if unsafe { hp2_core::object_model::is_a(field.cast(), property_class.cast()) } {
                let property = field.cast::<UProperty>();
                if unsafe { hp2_core::object_model::is_a(field.cast(), object_property_class.cast()) } {
                    let offset = unsafe { (*property).offset } as usize;
                    let array_dim = unsafe { (*property).array_dim };
                    for index in 0..array_dim {
                        out.push(base + offset + index as usize * std::mem::size_of::<*mut UObject>());
                    }
                } else if unsafe { (*field.cast::<UObject>()).class } == struct_property_class.cast() {
                    let nested = unsafe { (*field.cast::<UStructProperty>()).r#struct.cast::<UStruct>() };
                    let offset = unsafe { (*property).offset };
                    let element_size = unsafe { (*property).element_size };
                    let array_dim = unsafe { (*property).array_dim };
                    for index in 0..array_dim {
                        unsafe {
                            collect_reference_slots(
                                nested,
                                base + (offset + index * element_size) as usize,
                                property_class,
                                object_property_class,
                                struct_property_class,
                                out,
                            );
                        }
                    }
                }
            }
            field = unsafe { (*field).next.cast() };
        }
        current = unsafe { (*current.cast::<UField>()).super_field.cast() };
    }
}

/// One slot of `cleanup_reference_property`: null a reference to a deleted actor.
unsafe fn clear_deleted_actor_reference(
    access: CoreAccess<'_>,
    slot: *mut *mut UObject,
    actor_class: *mut hp2_abi::engine::generated::UClass,
    modify: bool,
) -> RuntimeResult<()> {
    let referenced = unsafe { slot.read_unaligned() };
    if referenced.is_null()
        || !unsafe {
            hp2_core::object_model::is_a(referenced.cast(), actor_class.cast())
                && (*referenced.cast::<AActor>()).b_delete_me()
        }
    {
        return Ok(());
    }
    if modify {
        unsafe { access.modify_object(referenced)?; }
    }
    unsafe { slot.write_unaligned(ptr::null_mut()); }
    Ok(())
}

unsafe fn cleanup_destroyed_refs(
    access: CoreAccess<'_>,
    structure: *mut UStruct,
    data: *mut u8,
    editor: bool,
    property_class: *mut hp2_abi::engine::generated::UClass,
    object_property_class: *mut hp2_abi::engine::generated::UClass,
    struct_property_class: *mut hp2_abi::engine::generated::UClass,
    actor_class: *mut hp2_abi::engine::generated::UClass,
) -> RuntimeResult<()> {
    if editor {
        let mut current = structure;
        while !current.is_null() {
            let mut field = unsafe { (*current).children.cast::<UField>() };
            while !field.is_null() {
                if unsafe { hp2_core::object_model::is_a(field.cast(), property_class.cast()) } {
                    let property = field.cast::<UProperty>();
                    if unsafe {
                        hp2_core::object_model::is_a(field.cast(), object_property_class.cast())
                    } {
                        unsafe {
                            cleanup_reference_property(access, data, property, actor_class, true)?;
                        }
                    } else if unsafe { (*field.cast::<UObject>()).class }
                        == struct_property_class.cast()
                    {
                        let nested = unsafe {
                            (*field.cast::<UStructProperty>()).r#struct.cast::<UStruct>()
                        };
                        let offset = unsafe { (*property).offset };
                        let element_size = unsafe { (*property).element_size };
                        let array_dim = unsafe { (*property).array_dim };
                        for index in 0..array_dim {
                            let nested_data = unsafe {
                                data.add((offset + index * element_size) as usize)
                            };
                            unsafe {
                                cleanup_destroyed_refs(
                                    access, nested, nested_data, true, property_class,
                                    object_property_class, struct_property_class, actor_class,
                                )?;
                            }
                        }
                    }
                }
                field = unsafe { (*field).next.cast() };
            }
            current = unsafe { (*current.cast::<UField>()).super_field.cast() };
        }
    } else {
        let mut reference = unsafe { (*structure).ref_link.cast::<UObjectProperty>() };
        while !reference.is_null() {
            let property = reference.cast::<UProperty>();
            unsafe { cleanup_reference_property(access, data, property, actor_class, false)?; }
            reference = unsafe { (*reference).next_reference.cast() };
        }
        let mut structure_property = unsafe { (*structure).struct_link.cast::<UStructProperty>() };
        while !structure_property.is_null() {
            let property = structure_property.cast::<UProperty>();
            let offset = unsafe { (*property).offset };
            let element_size = unsafe { (*property).element_size };
            let array_dim = unsafe { (*property).array_dim };
            let nested = unsafe { (*structure_property).r#struct.cast::<UStruct>() };
            for index in 0..array_dim {
                let element = unsafe { data.add((offset + index * element_size) as usize) };
                unsafe {
                    cleanup_destroyed_refs(
                        access, nested, element, false, property_class,
                        object_property_class, struct_property_class, actor_class,
                    )?;
                }
            }
            structure_property = unsafe { (*structure_property).next_struct.cast() };
        }
    }
    Ok(())
}

unsafe fn cleanup_reference_property(
    access: CoreAccess<'_>,
    data: *mut u8,
    property: *mut UProperty,
    actor_class: *mut hp2_abi::engine::generated::UClass,
    modify: bool,
) -> RuntimeResult<()> {
    let offset = unsafe { (*property).offset };
    let array_dim = unsafe { (*property).array_dim };
    let slot_base = unsafe { data.add(offset as usize).cast::<*mut UObject>() };
    for index in 0..array_dim {
        let slot = unsafe { slot_base.add(index as usize) };
        let referenced = unsafe { slot.read_unaligned() };
        if referenced.is_null()
            || !unsafe {
                hp2_core::object_model::is_a(referenced.cast(), actor_class.cast())
                    && (*referenced.cast::<AActor>()).b_delete_me()
            }
        {
            continue;
        }
        if modify {
            unsafe { access.modify_object(referenced)?; }
        }
        unsafe { slot.write_unaligned(ptr::null_mut()); }
    }
    Ok(())
}

/// Removes references to deleted actors from the active level objects.
///
/// # Safety
/// `level` is a live Core-owned ULevel record.
pub unsafe fn cleanup_destroyed(access: CoreAccess<'_>, level: *mut ULevel) -> RuntimeResult<()> {
    if level.is_null() {
        return Err(RuntimeError::new("Null canonical ULevel"));
    }
    unsafe { cleanup_destroyed_direct(access, level) }
}
