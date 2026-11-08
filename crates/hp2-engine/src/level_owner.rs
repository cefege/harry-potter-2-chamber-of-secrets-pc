//! Canonical class operations for Engine levels.

use crate::{collision_owner::CollisionOwner, url_owner::UrlOwner};
use hp2_abi::engine::generated::{
    AActor, FCheckResult, FRotator,
    FReachSpec, FStringMapPair, TArrayRaw, TMapStringString, TTransArrayRaw, ULevel, ULevelBase,
    UClass, UObject,
};
use hp2_abi::FVector;
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    map_storage::{self, Hp2MapServices, Hp2MapSlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    object_archive::{ActorSlotCompactIndexTrace, ArchiveMode, ObjectArchive},
    object_owner::ClassOps,
};
use std::{alloc::Layout, ffi::c_void, mem, ptr, rc::Rc};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_ABSTRACT: u32 = 1;
const LEVEL_BASE_SIZE: usize = 224;
const _: () = {
    assert!(mem::size_of::<ULevelBase>() == LEVEL_BASE_SIZE);
    assert!(mem::offset_of!(ULevelBase, actors) == 80);
    assert!(mem::offset_of!(ULevelBase, net_driver) == 104);
    assert!(mem::offset_of!(ULevelBase, engine) == 112);
    assert!(mem::offset_of!(ULevelBase, url) == 120);
    assert!(mem::offset_of!(ULevelBase, demo_rec_driver) == 216);
    assert!(mem::size_of::<TMapStringString>() == 32);
    assert!(mem::offset_of!(TMapStringString, pairs) == 0);
    assert!(mem::offset_of!(TMapStringString, hash) == 16);
    assert!(mem::offset_of!(TMapStringString, hash_count) == 24);
    assert!(mem::size_of::<FStringMapPair>() == 40);
    assert!(mem::offset_of!(FStringMapPair, hash_next) == 0);
    assert!(mem::offset_of!(FStringMapPair, key) == 8);
    assert!(mem::offset_of!(FStringMapPair, value) == 24);
    assert!(mem::size_of::<FReachSpec>() == 40);
    assert!(mem::offset_of!(FReachSpec, distance) == 0);
    assert!(mem::offset_of!(FReachSpec, start) == 8);
    assert!(mem::offset_of!(FReachSpec, end) == 16);
    assert!(mem::offset_of!(FReachSpec, collision_radius) == 24);
    assert!(mem::offset_of!(FReachSpec, collision_height) == 28);
    assert!(mem::offset_of!(FReachSpec, reach_flags) == 32);
    assert!(mem::offset_of!(FReachSpec, b_pruned) == 36);
};


/// Native Engine class specifications for ULevelBase and ULevel.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "LevelBase",
        package: "Engine",
        super_name: Some("Object"),
        within: "Object",
        flags: CLASS_ABSTRACT,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<ULevelBase>()),
        operations: Some(LEVEL_BASE_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
    NativeClassSpec {
        short_name: "Level",
        package: "Engine",
        super_name: Some("LevelBase"),
        within: "Object",
        flags: 0,
        object_flags: CLASS_OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<ULevel>()),
        operations: Some(LEVEL_OPERATIONS),
        properties: &[],
        defaults: &[],
    },
];

pub(crate) const LEVEL_BASE_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.construct = construct_level_base;
    operations.destroy = destroy_level_base;
    operations.finalize_members = finalize_level_base_members;
    operations.serialize = serialize_level_base;
    operations.visit_references = visit_level_base;
    operations
};

const LEVEL_OPERATIONS: ClassOps = {
    let mut operations = LEVEL_BASE_OPERATIONS;
    operations.construct = construct_level;
    operations.destroy = destroy_level;
    operations.finalize_members = finalize_level_members;
    operations.serialize = serialize_level;
    operations.post_load = post_load_level;
    operations.visit_references = visit_level;
    operations
};

struct CoreLevelPostLoad<'a> {
    access: CoreAccess<'a>,
    class: *mut UClass,
    index: i32,
}
impl crate::level::LevelPostLoadOps for CoreLevelPostLoad<'_> {
    type Error = RuntimeError;
    fn current_actor(&mut self) -> RuntimeResult<*mut c_void> {
        Ok(if self.index < self.access.object_count() {
            self.access.object_at(self.index).cast()
        } else { ptr::null_mut() })
    }
    unsafe fn object_outer(&mut self, object: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { ptr::addr_of!((*object.cast::<UObject>()).outer).read_unaligned() })
    }
    unsafe fn set_xlevel(&mut self, actor: *mut c_void, level: *mut c_void) -> RuntimeResult<()> {
        unsafe { ptr::addr_of_mut!((*actor.cast::<AActor>()).x_level).write_unaligned(level); }
        Ok(())
    }
    fn advance_actor(&mut self) -> RuntimeResult<()> {
        unsafe { self.access.next_object_index(&mut self.index, self.class)?; }
        Ok(())
    }
}

unsafe fn post_load_level(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.post_load)(access, object)?; }
    let mut actors = CoreLevelPostLoad {
        access, class: access.native_class("Engine.Actor")?, index: -1,
    };
    unsafe { access.next_object_index(&mut actors.index, actors.class)?; }
    unsafe { crate::level::level_post_load_with(&mut actors, object.cast()) }
}


unsafe fn construct_level_base(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (OBJECT.construct)(access, object)?; }
    let base = object.cast::<u8>();
    unsafe { ptr::write_bytes(base.add(mem::size_of::<UObject>()), 0, LEVEL_BASE_SIZE - mem::size_of::<UObject>()); }
    let level_base = object.cast::<ULevelBase>();
    unsafe { (*level_base).actors.owner = object.cast(); }
    let defaults = UrlOwner::current_defaults(access)?;
    unsafe { UrlOwner::construct_default(access, &defaults, ptr::addr_of_mut!((*level_base).url))?; }
    Ok(())
}

unsafe fn construct_level(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { construct_level_base(access, object)?; }
    let level = object.cast::<ULevel>();
    unsafe {
        ptr::write_bytes(object.cast::<u8>().add(LEVEL_BASE_SIZE), 0, mem::size_of::<ULevel>() - LEVEL_BASE_SIZE);
        (*level).model = ptr::null_mut();
        (*level).hash = ptr::null_mut();
        (*level).brush_tracker = ptr::null_mut();
        (*level).first_deleted = ptr::null_mut();
        (*level).newly_spawned = ptr::null_mut();
        (*level).travel_info.hash_count = 8;
        travel_map_rehash(ptr::addr_of_mut!((*level).travel_info))?;
    }
    Ok(())
}

pub(crate) unsafe fn destroy_level_base_body(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let base = object.cast::<ULevelBase>();
    for slot in [
        unsafe { ptr::addr_of_mut!((*base).net_driver) },
        unsafe { ptr::addr_of_mut!((*base).demo_rec_driver) },
    ] {
        let driver = unsafe { slot.read_unaligned() };
        if !driver.is_null() {
            unsafe {
                access.delete_object(driver.cast())?;
                slot.write_unaligned(ptr::null_mut());
            }
        }
    }
    unsafe { (OBJECT.destroy)(access, object) }
}

unsafe fn finalize_level_base_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let base = object.cast::<ULevelBase>();
    let url_result = unsafe { UrlOwner::destroy(access, ptr::addr_of_mut!((*base).url)) };
    let actors_result = unsafe {
        array_storage::release(
            array_slots(ptr::addr_of_mut!((*base).actors).cast()),
            array_storage::system_allocator(),
        )
    }.map_err(|code| RuntimeError::new(&format!("Unable to release Level Actors array ({code})")));
    url_result.and(actors_result)
}

unsafe fn destroy_level_base(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { destroy_level_base_body(access, object) }
}

unsafe fn destroy_level(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let level = object.cast::<ULevel>();
    let hash = unsafe { destroy_hash(level) };
    let tracker = unsafe { crate::dynbsp_owner::destroy(access, level) };
    let body = unsafe { destroy_level_base_body(access, object) };
    hash.and(tracker).and(body)
}

unsafe fn finalize_level_members(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let level = object.cast::<ULevel>();
    let travel = unsafe { destroy_travel_map(access, ptr::addr_of_mut!((*level).travel_info)) };
    let reach = unsafe {
        array_storage::release(
            array_slots(ptr::addr_of_mut!((*level).reach_specs)),
            array_storage::system_allocator(),
        )
    }.map_err(|code| RuntimeError::new(&format!("Unable to release Level ReachSpecs array ({code})")));
    let base = unsafe { finalize_level_base_members(access, object) };
    travel.and(reach).and(base)
}


unsafe fn collision_owner(level: *mut ULevel) -> RuntimeResult<Rc<CollisionOwner>> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in collision operation")); }
    let pointer = unsafe { (*level).hash.cast::<CollisionOwner>() };
    if pointer.is_null() { return Err(RuntimeError::new("Level has no canonical collision hash")); }
    // SAFETY: this private slot owns one Rc; retain it before any reentrant operation.
    unsafe { Rc::increment_strong_count(pointer); Ok(Rc::from_raw(pointer)) }
}

unsafe fn destroy_hash(level: *mut ULevel) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in collision destruction")); }
    let slot = unsafe { ptr::addr_of_mut!((*level).hash) };
    let pointer = unsafe { slot.read_unaligned().cast::<CollisionOwner>() };
    unsafe { slot.write_unaligned(ptr::null_mut()); }
    if !pointer.is_null() {
        // SAFETY: release exactly the private Level slot's ownership, not query scratch.
        unsafe { drop(Rc::from_raw(pointer)); }
    }
    Ok(())
}

/// Reads the current canonical Actor array length and its storage invariant.
///
/// # Safety
/// Level and its actor-array header are live in this Core owner.
pub unsafe fn actor_count(level: *mut ULevel) -> RuntimeResult<i32> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in actor count")); }
    let actors = unsafe { ptr::addr_of!((*level).base.actors).cast::<TArrayRaw>() };
    let count = unsafe { (*actors).num };
    let capacity = unsafe { (*actors).max };
    let data = unsafe { (*actors).data };
    if count < 0 || capacity < count || (count > 0 && data.is_null()) {
        return Err(RuntimeError::new("Invalid Level Actors array"));
    }
    Ok(count)
}

/// Reads a canonical Actor identity without retaining an array borrow over callbacks.
///
/// # Safety
/// Level is live and index is within its current actor_count; the array remains live.
pub unsafe fn actor_at(level: *mut ULevel, index: i32) -> *mut AActor {
    unsafe { (*level).base.actors.data.cast::<*mut AActor>().add(index as usize).read_unaligned() }
}

/// Enables or disables actor collision using the shared native actor iteration policy.
///
/// # Safety
/// Level, all reached Actors, and their selected primitive objects are canonical and live.
pub unsafe fn set_actor_collision(access: CoreAccess<'_>, level: *mut ULevel, enabled: bool) -> RuntimeResult<()> {
    if level.is_null() { return Err(RuntimeError::new("Null Level in SetActorCollision")); }
    if enabled && unsafe { (*level).hash.is_null() } {
        let hash = Rc::new(CollisionOwner::new(access)?);
        unsafe { (*level).hash = Rc::into_raw(hash.clone()).cast_mut().cast(); }
        crate::level::for_colliding_actors(
            || unsafe { actor_count(level) },
            |index| Ok(unsafe { actor_at(level, index).cast() }),
            |actor| Ok(unsafe { (*actor.cast::<AActor>()).b_collide_actors() }),
            |actor| unsafe { hash.add_actor(access, actor.cast()) },
        )?;
    } else if !enabled && unsafe { !(*level).hash.is_null() } {
        crate::level::for_colliding_actors(
            || unsafe { actor_count(level) },
            |index| Ok(unsafe { actor_at(level, index).cast() }),
            |actor| Ok(unsafe { (*actor.cast::<AActor>()).b_collide_actors() }),
            |actor| unsafe { hash_remove(access, level, actor.cast(), false) },
        )?;
        unsafe { destroy_hash(level)?; }
    }
    Ok(())
}

/// Inserts a live canonical Actor into its Level's active hash.
///
/// # Safety
/// Level, Actor, selected primitive and Engine references remain live through callbacks.
pub unsafe fn hash_add(access: CoreAccess<'_>, level: *mut ULevel, actor: *mut AActor) -> RuntimeResult<()> {
    let hash = unsafe { collision_owner(level)? };
    unsafe { hash.add_actor(access, actor) }
}

/// Removes a live canonical Actor from its old hash cells.
///
/// # Safety
/// Level and Actor remain live through callbacks; the hash is the private canonical handle.
pub unsafe fn hash_remove(access: CoreAccess<'_>, level: *mut ULevel, actor: *mut AActor, thorough: bool) -> RuntimeResult<()> {
    let hash = unsafe { collision_owner(level)? };
    unsafe { hash.remove_actor(access, actor, thorough) }
}

/// Verifies that the Level's canonical collision hash no longer references an actor.
///
/// # Safety
/// The Level owns a live collision hash and `actor` remains live for this call.
pub unsafe fn hash_check_actor_not_referenced(
    level: *mut ULevel,
    actor: *mut AActor,
) -> RuntimeResult<()> {
    unsafe { collision_owner(level)? }.check_actor_not_referenced(actor)
}

/// Resets native per-tick hash counters without changing actor links.
///
/// # Safety
/// Level owns a live private canonical collision handle on this engine thread.
pub unsafe fn hash_tick(level: *mut ULevel) -> RuntimeResult<()> {
    unsafe { collision_owner(level)? }.tick();
    Ok(())
}

/// Returns point-overlap results in Core GMem, bounded by the caller's scratch mark.
///
/// # Safety
/// Level, hashed Actors and primitives remain live through the query and result use.
pub unsafe fn hash_point_check(access: CoreAccess<'_>, level: *mut ULevel, location: FVector, extent: FVector) -> RuntimeResult<*mut FCheckResult> {
    let hash = unsafe { collision_owner(level)? };
    unsafe { hash.point_check(access, location, extent) }
}

/// Resolves `ULevel::GetZoneActor`, including its LevelInfo fallback for
/// model zones with no dedicated ZoneInfo actor.
///
/// # Safety
/// Level, its model, and the model's referenced zone actors remain live.
pub(crate) unsafe fn zone_actor(
    level: *mut ULevel, index: i32,
) -> RuntimeResult<*mut hp2_abi::engine::generated::AZoneInfo> {
    let model = unsafe { (*level).model.cast::<hp2_abi::engine::generated::UModel>() };
    if model.is_null() { return Err(RuntimeError::new("Level has no model for zone actor lookup")); }
    let count = unsafe { (*model).zones.len() } / std::mem::size_of::<hp2_abi::render_scene::FZoneProperties>();
    if index < 0 || index as usize >= count {
        return Err(RuntimeError::new("Level zone actor index is out of range"));
    }
    let actor = unsafe {
        let zones = ptr::addr_of!((*model).zones).cast::<hp2_abi::render_scene::FZoneProperties>();
        ptr::addr_of!((*zones.add(index as usize)).zone_actor).read_unaligned()
    };
    if actor.is_null() {
        unsafe { Ok(crate::package_map_level_owner::level_info(level)?.cast()) }
    } else {
        Ok(actor.cast())
    }
}

/// Resolves a world point through the Level's canonical UModel BSP region tables.
///
/// # Safety
/// `level`, its model, and every zone actor referenced by that model remain live.
pub(crate) unsafe fn model_point_region(
    level: *mut ULevel,
    fallback_zone: *mut c_void,
    location: FVector,
) -> RuntimeResult<hp2_abi::engine::generated::FPointRegion> {
    let model = unsafe { (*level).model.cast::<hp2_abi::engine::generated::UModel>() };
    if model.is_null() {
        return Err(RuntimeError::new("Level has no canonical model"));
    }
    unsafe { CollisionOwner::model_point_region(model, fallback_zone, location) }
}

/// Returns radius matches in Core GMem; native radius results initialize Actor/Next only.
///
/// # Safety
/// Level and hashed Actors are live; only Actor/Next are read before the caller pops its mark.
pub unsafe fn hash_radius_check(access: CoreAccess<'_>, level: *mut ULevel, location: FVector, radius: f32) -> RuntimeResult<*mut FCheckResult> {
    let hash = unsafe { collision_owner(level)? };
    unsafe { hash.radius_check(access, location, radius) }
}

/// Returns segment results in the caller-selected scratch owner.
///
/// # Safety
/// Level, hashed Actors and primitives remain live; memory and its active mark outlive results.
pub unsafe fn hash_line_check(access: CoreAccess<'_>, level: *mut ULevel, memory: &hp2_core::mem_owner::MemStackOwner,
    end: FVector, start: FVector, extent: FVector, flags: u32,
) -> RuntimeResult<*mut FCheckResult> {
    let hash = unsafe { collision_owner(level)? };
    unsafe { hash.line_check(access, memory, end, start, extent, flags) }
}

/// Checks an exchanged Actor pose against the Level's actual hashed primitives.
///
/// # Safety
/// Level and all reached Actors remain live; results are bounded by the caller's GMem mark.
pub unsafe fn hash_encroachment_check(access: CoreAccess<'_>, level: *mut ULevel, actor: *mut AActor,
    location: FVector, rotation: FRotator,
) -> RuntimeResult<*mut FCheckResult> {
    let hash = unsafe { collision_owner(level)? };
    unsafe { hash.encroachment_check(access, actor, location, rotation) }
}
fn actor_slot_dump_capture_enabled(access: CoreAccess<'_>, archive: &ObjectArchive<'_>) -> RuntimeResult<bool> {
    if !archive.is_loading()
        || archive.is_trans()
        || !std::env::var_os("HP2_ACTOR_SLOT_DUMP").is_some_and(|value| !value.is_empty())
    {
        return Ok(false);
    }
    crate::actor_slot_dump_available(access)
}

unsafe fn serialize_actor_slot(
    access: CoreAccess<'_>,
    level: *mut ULevelBase,
    archive: &ObjectArchive<'_>,
    slot: *mut *mut UObject,
    index: i32,
    capture: bool,
) -> RuntimeResult<()> {
    let reference_before = if capture {
        i32::try_from(archive.tell_file()?)
            .map_err(|_| RuntimeError::new("Level actor reference offset exceeds the INT range"))?
    } else {
        -1
    };
    let capture_compact = capture && archive.supports_actor_slot_compact_index_trace();
    let mut trace = ActorSlotCompactIndexTrace {
        index: -1,
        offset_after: -1,
        captured: 0,
    };
    if capture_compact {
        unsafe {
            archive.with_actor_slot_compact_index_trace(
                ptr::addr_of_mut!(trace),
                || archive.serialize_object_slot(slot),
            )?;
        }
    } else {
        unsafe { archive.serialize_object_slot(slot)?; }
    }
    if capture {
        crate::observe_actor_slot_dump(
            access,
            crate::ActorSlotDumpEvent::RawActor {
                level: level.cast(),
                slot: index,
                actor: unsafe { ptr::read_unaligned(slot) }.cast(),
                reference_offset_before: reference_before,
                reference_offset_after: trace.offset_after,
                compact_package_index: trace.index,
                compact_package_index_captured: trace.captured != 0,
                compact_package_index_supported: capture_compact,
            },
        )?;
    }
    Ok(())
}

unsafe fn serialize_actors(
    access: CoreAccess<'_>,
    level: *mut ULevelBase,
    archive: &ObjectArchive<'_>,
    actors: *mut TTransArrayRaw,
) -> RuntimeResult<()> {
    if archive.is_trans() {
        return Ok(());
    }
    let array = actors.cast::<TArrayRaw>();
    let old_num = unsafe { (*array).num };
    let old_max = unsafe { (*array).max };
    if old_num < 0 || old_max < old_num || (old_num != 0 && unsafe { (*array).data.is_null() }) {
        return Err(RuntimeError::new("Invalid Level Actors array"));
    }
    if !matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
        let width = mem::size_of::<*mut UObject>();
        archive.count_bytes(
            (old_num as usize).checked_mul(width).ok_or_else(|| RuntimeError::new("Level Actors byte count overflow"))?,
            (old_max as usize).checked_mul(width).ok_or_else(|| RuntimeError::new("Level Actors capacity byte count overflow"))?,
        )?;
        let mut count = old_num;
        archive.serialize_i32(&mut count)?;
        let mut legacy_max = old_num;
        archive.serialize_i32(&mut legacy_max)?;
        if archive.is_loading() {
            if count < 0 { return Err(RuntimeError::new("Invalid serialized Level Actors count")); }
            let allocator = array_storage::system_allocator();
            let mut staged = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            let staged_slots = array_slots(&mut staged);
            if let Err(code) = unsafe { array_storage::set(staged_slots, count, width as i32, ptr::null(), allocator) } {
                let _ = unsafe { array_storage::release(staged_slots, allocator) };
                return Err(RuntimeError::new(&format!("Unable to allocate Level Actors ({code})")));
            }
            let capture = actor_slot_dump_capture_enabled(access, archive)?;
            if capture {
                crate::observe_actor_slot_dump(
                    access,
                    crate::ActorSlotDumpEvent::BeginRawTable { level: level.cast() },
                )?;
            }
            let read_result = (|| {
                for index in 0..count as usize {
                    let slot = unsafe { staged.data.cast::<*mut UObject>().add(index) };
                    unsafe {
                        serialize_actor_slot(
                            access,
                            level,
                            archive,
                            slot,
                            i32::try_from(index).map_err(|_| RuntimeError::new("Level actor slot exceeds the INT range"))?,
                            capture,
                        )?;
                    }
                }
                if capture {
                    crate::observe_actor_slot_dump(
                        access,
                        crate::ActorSlotDumpEvent::EndRawTable { level: level.cast() },
                    )?;
                }
                Ok(())
            })();
            if let Err(error) = read_result {
                let _ = unsafe { array_storage::release(staged_slots, allocator) };
                return Err(error);
            }
            let old = unsafe { ptr::replace(array, staged) };
            staged = old;
            unsafe { array_storage::release(array_slots(&mut staged), allocator) }
                .map_err(|code| RuntimeError::new(&format!("Unable to release replaced Level Actors ({code})")))?;
            return Ok(());
        }
    }
    for index in 0..old_num as usize {
        let slot = unsafe { (*array).data.cast::<*mut UObject>().add(index) };
        unsafe {
            serialize_actor_slot(
                access,
                level,
                archive,
                slot,
                i32::try_from(index).map_err(|_| RuntimeError::new("Level actor slot exceeds the INT range"))?,
                false,
            )?;
        }
    }
    Ok(())
}

unsafe fn serialize_reach_spec(archive: &ObjectArchive<'_>, spec: *mut FReachSpec) -> RuntimeResult<()> {
    unsafe {
        if matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
            archive.serialize_object_slot(ptr::addr_of_mut!((*spec).start).cast())?;
            archive.serialize_object_slot(ptr::addr_of_mut!((*spec).end).cast())?;
        } else {
            archive.serialize_i32(&mut (*spec).distance)?;
            archive.serialize_object_slot(ptr::addr_of_mut!((*spec).start).cast())?;
            archive.serialize_object_slot(ptr::addr_of_mut!((*spec).end).cast())?;
            archive.serialize_i32(&mut (*spec).collision_radius)?;
            archive.serialize_i32(&mut (*spec).collision_height)?;
            archive.serialize_i32(&mut (*spec).reach_flags)?;
            archive.serialize_u8(&mut (*spec).b_pruned)?;
        }
    }
    Ok(())
}

unsafe fn serialize_reach_specs(archive: &ObjectArchive<'_>, array: *mut TArrayRaw) -> RuntimeResult<()> {
    let references_only = matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports);
    let width = mem::size_of::<FReachSpec>();
    let old_num = unsafe { (*array).num };
    let old_max = unsafe { (*array).max };
    if old_num < 0 || old_max < old_num || (old_num != 0 && unsafe { (*array).data.is_null() }) {
        return Err(RuntimeError::new("Invalid Level ReachSpecs array"));
    }
    archive.count_bytes(
        (old_num as usize).checked_mul(width).ok_or_else(|| RuntimeError::new("Level ReachSpecs byte count overflow"))?,
        (old_max as usize).checked_mul(width).ok_or_else(|| RuntimeError::new("Level ReachSpecs capacity byte count overflow"))?,
    )?;
    if !references_only {
        let mut count = old_num;
        archive.serialize_compact_index(&mut count)?;
        if archive.is_loading() {
            if count < 0 { return Err(RuntimeError::new("Invalid serialized Level ReachSpecs count")); }
            let allocator = array_storage::system_allocator();
            let mut staged = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            let staged_slots = array_slots(&mut staged);
            if let Err(code) = unsafe { array_storage::set(staged_slots, count, width as i32, ptr::null(), allocator) } {
                let _ = unsafe { array_storage::release(staged_slots, allocator) };
                return Err(RuntimeError::new(&format!("Unable to allocate Level ReachSpecs ({code})")));
            }
            let read_result = (|| {
                for index in 0..count as usize {
                    let spec = unsafe { staged.data.cast::<FReachSpec>().add(index) };
                    unsafe { serialize_reach_spec(archive, spec)?; }
                }
                Ok(())
            })();
            if let Err(error) = read_result {
                let _ = unsafe { array_storage::release(staged_slots, allocator) };
                return Err(error);
            }
            let old = unsafe { ptr::replace(array, staged) };
            staged = old;
            unsafe { array_storage::release(array_slots(&mut staged), allocator) }
                .map_err(|code| RuntimeError::new(&format!("Unable to release replaced Level ReachSpecs ({code})")))?;
            return Ok(());
        }
    }
    for index in 0..old_num as usize {
        let spec = unsafe { (*array).data.cast::<FReachSpec>().add(index) };
        unsafe { serialize_reach_spec(archive, spec)?; }
    }
    Ok(())
}

unsafe fn serialize_level_base(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.serialize)(access, object, archive)?; }
    let base = object.cast::<ULevelBase>();
    let actors = unsafe { ptr::addr_of_mut!((*base).actors) };
    if unsafe { (*actors).owner } != object.cast() {
        return Err(RuntimeError::new("Level Actors array has an invalid owner"));
    }
    unsafe {
        serialize_actors(access, base, archive, actors)?;
        UrlOwner::serialize(access, archive, ptr::addr_of_mut!((*base).url))?;
        if !archive.is_loading() && !archive.is_saving() {
            archive.serialize_object_slot(ptr::addr_of_mut!((*base).net_driver).cast())?;
            archive.serialize_object_slot(ptr::addr_of_mut!((*base).demo_rec_driver).cast())?;
        }
    }
    Ok(())
}

unsafe fn serialize_level(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    let level = object.cast::<ULevel>();
    unsafe { serialize_level_base(access, object, archive)?; }
    unsafe {
        archive.serialize_object_slot(ptr::addr_of_mut!((*level).model).cast())?;
        serialize_reach_specs(archive, ptr::addr_of_mut!((*level).reach_specs))?;
        serialize_time_seconds(archive, ptr::addr_of_mut!((*level).time_seconds))?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*level).first_deleted).cast())?;
        for block in &mut (*level).text_blocks {
            archive.serialize_object_slot(ptr::from_mut(block).cast())?;
        }
        if archive.flags().version > 62 {
            serialize_travel_map(access, archive, ptr::addr_of_mut!((*level).travel_info))?;
        } else if archive.flags().version >= 61 {
            serialize_legacy_travel_info(access, archive, ptr::addr_of_mut!((*level).travel_info))?;
        }
        if !(*level).model.is_null() && !archive.is_trans() {
            access.preload((*level).model.cast::<UObject>())?;
        }
    }
    if !unsafe { (*level).brush_tracker }.is_null() {
        unsafe { crate::dynbsp_owner::count_bytes(level, archive)?; }
    }
    Ok(())
}

unsafe fn serialize_time_seconds(archive: &ObjectArchive<'_>, time: *mut [u8; 8]) -> RuntimeResult<()> {
    let mut seconds = hp2_core::time::get_float(unsafe { ptr::read_unaligned(time.cast::<i64>()) });
    archive.serialize_f32(&mut seconds)?;
    if archive.is_loading() {
        unsafe { ptr::write_unaligned(time.cast::<i64>(), hp2_core::time::from_float(seconds)); }
    }
    Ok(())
}

unsafe fn visit_level_base(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.visit_references)(access, object, archive)?; }
    let base = object.cast::<ULevelBase>();
    let actors = unsafe { ptr::addr_of_mut!((*base).actors) };
    if unsafe { (*actors).owner } != object.cast() {
        return Err(RuntimeError::new("Level Actors array has an invalid owner"));
    }
    unsafe {
        serialize_actors(access, base, archive, actors)?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*base).net_driver).cast())?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*base).demo_rec_driver).cast())?;
    }
    Ok(())
}

unsafe fn visit_level(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { visit_level_base(access, object, archive)?; }
    let level = object.cast::<ULevel>();
    unsafe {
        archive.serialize_object_slot(ptr::addr_of_mut!((*level).model).cast())?;
        serialize_reach_specs(archive, ptr::addr_of_mut!((*level).reach_specs))?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*level).first_deleted).cast())?;
        for block in &mut (*level).text_blocks {
            archive.serialize_object_slot(ptr::from_mut(block).cast())?;
        }
    }
    Ok(())
}


fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe { Hp2ArraySlots {
        data: ptr::addr_of_mut!((*array).data),
        num: ptr::addr_of_mut!((*array).num),
        max: ptr::addr_of_mut!((*array).max),
    } }
}
unsafe fn copy_fstring(access: CoreAccess<'_>, destination: *mut TArrayRaw, source: *const TArrayRaw) -> RuntimeResult<()> {
    let source = unsafe { source.read_unaligned() };
    if source.num < 0 || (source.num > 0 && source.data.is_null()) {
        return Err(RuntimeError::new("Invalid Level TravelInfo FString"));
    }
    if source.num <= 1 {
        return unsafe { access.assign_fstring(destination, &[0]) };
    }
    let terminated = unsafe { std::slice::from_raw_parts(source.data.cast::<u32>(), source.num as usize) };
    if terminated.last() != Some(&0) {
        return Err(RuntimeError::new("Invalid Level TravelInfo FString terminator"));
    }
    unsafe { access.assign_fstring(destination, terminated) }
}

unsafe fn serialize_legacy_travel_info(
    access: CoreAccess<'_>,
    archive: &ObjectArchive<'_>,
    map: *mut TMapStringString,
) -> RuntimeResult<()> {
    if matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
        return Ok(());
    }
    let mut names = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let mut items = names;
    let result = (|| {
        if !archive.is_loading() {
            let count = unsafe { (*map).pairs.num };
            if count < 0 || (count > 0 && unsafe { (*map).pairs.data.is_null() }) {
                return Err(RuntimeError::new("Invalid Level TravelInfo pairs"));
            }
            let allocator = array_storage::system_allocator();
            for array in [&mut names, &mut items] {
                let slots = array_slots(array);
                if let Err(code) = unsafe {
                    array_storage::set(slots, count, mem::size_of::<TArrayRaw>() as i32, ptr::null(), allocator)
                } {
                    unsafe { ptr::write_unaligned(array, TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
                    return Err(RuntimeError::new(&format!("Unable to allocate legacy TravelInfo strings ({code})")));
                }
                if count > 0 {
                    unsafe { ptr::write_bytes(array.data.cast::<TArrayRaw>(), 0, count as usize); }
                }
            }
            let pairs = unsafe { (*map).pairs.data.cast::<FStringMapPair>() };
            for index in 0..count as usize {
                let pair = unsafe { pairs.add(index) };
                unsafe {
                    copy_fstring(access, names.data.cast::<TArrayRaw>().add(index), ptr::addr_of!((*pair).key))?;
                    copy_fstring(access, items.data.cast::<TArrayRaw>().add(index), ptr::addr_of!((*pair).value))?;
                }
            }
        }
        unsafe {
            access.serialize_owned_fstring_array(archive, &mut names)?;
            access.serialize_owned_fstring_array(archive, &mut items)?;
        }
        if !archive.is_loading() { return Ok(()); }
        let name_count = names.num;
        if name_count != items.num {
            return Err(RuntimeError::new("Legacy Level TravelInfo name/value counts differ"));
        }
        let mut staged: TMapStringString = unsafe { mem::zeroed() };
        staged.hash_count = 8;
        let pair_width = mem::size_of::<FStringMapPair>() as i32;
        let allocator = array_storage::system_allocator();
        if let Err(code) = unsafe {
            array_storage::set(array_slots(ptr::addr_of_mut!(staged.pairs)), name_count, pair_width, ptr::null(), allocator)
        } {
            unsafe { ptr::write_unaligned(ptr::addr_of_mut!(staged.pairs), TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
            return Err(RuntimeError::new(&format!("Unable to allocate legacy TravelInfo map ({code})")));
        }
        let pairs = staged.pairs.data.cast::<FStringMapPair>();
        if name_count > 0 { unsafe { ptr::write_bytes(pairs, 0, name_count as usize); } }
        let names_data = names.data.cast::<TArrayRaw>();
        let items_data = items.data.cast::<TArrayRaw>();
        let build_result = (|| {
            let mut used = 0usize;
            for index in 0..name_count as usize {
                let name = unsafe { names_data.add(index) };
                let item = unsafe { items_data.add(index) };
                let name_raw = unsafe { name.read_unaligned() };
                if name_raw.num <= 0 || name_raw.data.is_null() {
                    return Err(RuntimeError::new("Invalid legacy TravelInfo name"));
                }
                let mut existing = None;
                for pair_index in 0..used {
                    let pair = unsafe { pairs.add(pair_index) };
                    let key = unsafe { ptr::addr_of!((*pair).key) };
                    let key_raw = unsafe { key.read_unaligned() };
                    if unsafe { hp2_core::ansi::stricmp(key_raw.data.cast(), name_raw.data.cast()) } == 0 {
                        existing = Some(pair);
                        break;
                    }
                }
                let pair = existing.unwrap_or_else(|| unsafe { pairs.add(used) });
                if existing.is_none() {
                    unsafe { copy_fstring(access, ptr::addr_of_mut!((*pair).key), name)?; }
                    used += 1;
                } else {
                    unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*pair).value))?; }
                }
                unsafe { copy_fstring(access, ptr::addr_of_mut!((*pair).value), item)?; }
            }
            staged.pairs.num = used as i32;
            unsafe { travel_map_rehash(&mut staged)?; }
            Ok(())
        })();
        if let Err(error) = build_result {
            let _ = unsafe { destroy_travel_map(access, &mut staged) };
            return Err(error);
        }
        let mut old = unsafe { ptr::replace(map, staged) };
        unsafe { destroy_travel_map(access, &mut old)?; }
        Ok(())
    })();
    let destroy_names = unsafe { access.destroy_owned_fstring_array(&mut names) };
    let destroy_items = unsafe { access.destroy_owned_fstring_array(&mut items) };
    result.and(destroy_names).and(destroy_items)
}

unsafe extern "C" fn travel_map_leaf(
    context: *mut std::ffi::c_void,
    op: i32,
    index: i32,
    argument: i32,
    out: *mut i32,
) -> i32 {
    if context.is_null() || out.is_null() || index < 0 {
        return 1;
    }
    let map = context.cast::<TMapStringString>();
    let pairs = unsafe { (*map).pairs.data.cast::<FStringMapPair>() };
    let count = unsafe { (*map).pairs.num };
    if index >= count && !matches!(op, map_storage::leaf::NUM | map_storage::leaf::EMPTY) {
        return 1;
    }
    let result = unsafe {
        match op {
            map_storage::leaf::EMPTY => {
                // Typed FString elements were destroyed by clear_travel_map.
                if array_storage::empty(
                    array_slots(ptr::addr_of_mut!((*map).pairs)), 0,
                    mem::size_of::<FStringMapPair>() as i32, ptr::null(),
                    array_storage::system_allocator(),
                ).is_err() { return 1; }
                0
            }
            map_storage::leaf::NUM => count,
            map_storage::leaf::NEXT => (*pairs.add(index as usize)).hash_next,
            map_storage::leaf::LINK => {
                (*pairs.add(index as usize)).hash_next = argument;
                0
            }
            map_storage::leaf::HASH => {
                let key = &(*pairs.add(index as usize)).key;
                if key.num < 0 || (key.num > 0 && key.data.is_null()) {
                    return 1;
                }
                let n = key.num.saturating_sub(1) as usize;
                let text = if n == 0 {
                    &[][..]
                } else {
                    std::slice::from_raw_parts(key.data.cast::<u32>(), n)
                };
                hp2_core::name::strihash_text(text) as i32
            }
            _ => return 1,
        }
    };
    unsafe { out.write_unaligned(result); }
    0
}

unsafe extern "C" fn travel_map_query(
    _context: *mut std::ffi::c_void,
    _op: i32,
    _index: i32,
    _argument: i32,
    _out: *mut i32,
) -> i32 {
    1
}

fn travel_map_services(map: *mut TMapStringString) -> Hp2MapServices {
    Hp2MapServices {
        context: map.cast(),
        leaf: travel_map_leaf,
        query: travel_map_query,
        allocator: array_storage::system_allocator(),
        tag: ptr::null(),
    }
}

unsafe fn travel_map_rehash(map: *mut TMapStringString) -> RuntimeResult<()> {
    let pairs = unsafe { (*map).pairs };
    let hash_count = unsafe { (*map).hash_count };
    if pairs.num < 0
        || pairs.max < pairs.num
        || (pairs.num > 0 && pairs.data.is_null())
        || hash_count < 8
        || !(hash_count as u32).is_power_of_two()
    {
        return Err(RuntimeError::new("Invalid Level TravelInfo map storage"));
    }
    let slots = unsafe {
        Hp2MapSlots {
            hash: ptr::addr_of_mut!((*map).hash).cast(),
            count: ptr::addr_of_mut!((*map).hash_count),
        }
    };
    unsafe { map_storage::rehash(slots, travel_map_services(map)) }
        .map_err(|code| RuntimeError::new(&format!("Unable to rehash Level TravelInfo ({code})")))
}

unsafe fn destroy_travel_pair_strings(
    access: CoreAccess<'_>, map: *mut TMapStringString, first: &mut Option<RuntimeError>,
) -> RuntimeResult<()> {
    let pairs = unsafe { (*map).pairs.data.cast::<FStringMapPair>() };
    let count = unsafe { (*map).pairs.num };
    if count < 0 || (count > 0 && pairs.is_null()) {
        return Err(RuntimeError::new("Invalid Level TravelInfo pairs"));
    }
    for index in 0..count as usize {
        let pair = unsafe { pairs.add(index) };
        if let Err(error) = unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*pair).key)) } {
            if first.is_none() { *first = Some(error); }
        }
        if let Err(error) = unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*pair).value)) } {
            if first.is_none() { *first = Some(error); }
        }
    }
    Ok(())
}

pub(crate) unsafe fn destroy_travel_map(access: CoreAccess<'_>, map: *mut TMapStringString) -> RuntimeResult<()> {
    let mut first = None;
    unsafe { destroy_travel_pair_strings(access, map, &mut first)?; }
    let allocator = array_storage::system_allocator();
    if let Err(code) = unsafe { array_storage::release(array_slots(ptr::addr_of_mut!((*map).pairs)), allocator) } {
        if first.is_none() {
            first = Some(RuntimeError::new(&format!("Unable to release Level TravelInfo pairs ({code})")));
        }
    }
    let hash = unsafe { (*map).hash.cast::<std::ffi::c_void>() };
    if !hash.is_null() && unsafe { (allocator.free)(allocator.context, hash) } != 0 && first.is_none() {
        first = Some(RuntimeError::new("Unable to release Level TravelInfo hash"));
    }
    unsafe {
        (*map).hash = ptr::null_mut();
        (*map).hash_count = 0;
    }
    first.map_or(Ok(()), Err)
}

/// Empties FString pairs and restores TMap's eight-bucket live empty state.
/// # Safety
/// The map remains live and exclusively available throughout the synchronous call.
pub(crate) unsafe fn clear_travel_map(
    access: CoreAccess<'_>, map: *mut TMapStringString,
) -> RuntimeResult<()> {
    let mut first = None;
    unsafe { destroy_travel_pair_strings(access, map, &mut first)?; }
    let slots = unsafe {
        Hp2MapSlots {
            hash: ptr::addr_of_mut!((*map).hash).cast(),
            count: ptr::addr_of_mut!((*map).hash_count),
        }
    };
    let result = unsafe { map_storage::run(slots, travel_map_services(map), map_storage::operation::EMPTY) }
        .map(|_| ())
        .map_err(|code| RuntimeError::new(&format!("Unable to empty Level TravelInfo ({code})")));
    first.map_or(result, Err)
}

/// Deeply assigns the native FString map, including independent string and hash storage.
/// # Safety
/// Both maps are live canonical storage; source strings remain live until copying ends.
pub(crate) unsafe fn assign_travel_map(
    access: CoreAccess<'_>, destination: *mut TMapStringString, source: *const TMapStringString,
) -> RuntimeResult<()> {
    if ptr::eq(destination.cast_const(), source) { return Ok(()); }
    let source = unsafe { source.read_unaligned() };
    if source.pairs.num < 0 || source.pairs.max < source.pairs.num
        || (source.pairs.num > 0 && source.pairs.data.is_null())
        || source.hash_count < 8 || !(source.hash_count as u32).is_power_of_two()
    { return Err(RuntimeError::new("Invalid source TravelInfo map")); }
    unsafe { destroy_travel_map(access, destination)?; }
    unsafe { (*destination).hash_count = source.hash_count; }
    let width = mem::size_of::<FStringMapPair>() as i32;
    unsafe {
        array_storage::add_zeroed(array_slots(ptr::addr_of_mut!((*destination).pairs)),
            source.pairs.num, width, ptr::null(), array_storage::system_allocator())
    }.map_err(|code| RuntimeError::new(&format!("Unable to copy TravelInfo pairs ({code})")))?;
    for index in 0..source.pairs.num as usize {
        let from = unsafe { source.pairs.data.cast::<FStringMapPair>().add(index) };
        let to = unsafe { (*destination).pairs.data.cast::<FStringMapPair>().add(index) };
        unsafe {
            copy_fstring(access, ptr::addr_of_mut!((*to).key), ptr::addr_of!((*from).key))?;
            copy_fstring(access, ptr::addr_of_mut!((*to).value), ptr::addr_of!((*from).value))?;
        }
    }
    unsafe { travel_map_rehash(destination) }
}

struct TravelMapSet<'a> {
    access: CoreAccess<'a>,
    map: *mut TMapStringString,
    key: &'a [u32],
    value: &'a [u32],
    error: Option<RuntimeError>,
}

unsafe extern "C" fn travel_map_set_leaf(
    context: *mut c_void, op: i32, index: i32, argument: i32, out: *mut i32,
) -> i32 {
    let context = unsafe { &*context.cast::<TravelMapSet<'_>>() };
    unsafe { travel_map_leaf(context.map.cast(), op, index, argument, out) }
}

unsafe extern "C" fn travel_map_set_query(
    context: *mut c_void, op: i32, index: i32, _argument: i32, out: *mut i32,
) -> i32 {
    let context = unsafe { &mut *context.cast::<TravelMapSet<'_>>() };
    let result = (|| -> RuntimeResult<i32> {
        if op == map_storage::query::HASH {
            return Ok(hp2_core::name::strihash_text(context.key) as i32);
        }
        if op == map_storage::query::CONSTRUCT {
            let index = unsafe { (*context.map).pairs.num };
            unsafe {
                array_storage::add_zeroed(array_slots(ptr::addr_of_mut!((*context.map).pairs)), 1,
                    mem::size_of::<FStringMapPair>() as i32, ptr::null(), array_storage::system_allocator())
            }.map_err(|code| RuntimeError::new(&format!("Unable to add TravelInfo pair ({code})")))?;
            let pair = unsafe { (*context.map).pairs.data.cast::<FStringMapPair>().add(index as usize) };
            // assign_fstring requires NUL-terminated input; the map key/value
            // slices arrive without a terminator (the key is explicitly
            // truncated in insert_travel_map), so terminate copies here.
            let mut key = context.key.to_vec();
            if key.last() != Some(&0) { key.push(0); }
            let mut value = context.value.to_vec();
            if value.last() != Some(&0) { value.push(0); }
            unsafe {
                context.access.assign_fstring(ptr::addr_of_mut!((*pair).key), &key)?;
                context.access.assign_fstring(ptr::addr_of_mut!((*pair).value), &value)?;
            }
            return Ok(index);
        }
        if index < 0 || index >= unsafe { (*context.map).pairs.num } {
            return Err(RuntimeError::new("Invalid TravelInfo pair index"));
        }
        let pair = unsafe { (*context.map).pairs.data.cast::<FStringMapPair>().add(index as usize) };
        match op {
            map_storage::query::KEY_EQ => {
                let key = unsafe { ptr::addr_of!((*pair).key).read_unaligned() };
                let length = key.num.saturating_sub(1) as usize;
                if key.num < 0 || key.max < key.num || (key.num > 0 && key.data.is_null()) {
                    return Err(RuntimeError::new("Invalid TravelInfo key"));
                }
                if length != context.key.len() { return Ok(0); }
                Ok(i32::from(length == 0 || unsafe {
                    hp2_core::ansi::strnicmp(key.data.cast(), context.key.as_ptr().cast(), length as i32) == 0
                }))
            }
            map_storage::query::ASSIGN => {
                unsafe { context.access.assign_fstring(ptr::addr_of_mut!((*pair).value), context.value)?; }
                Ok(0)
            }
            _ => Err(RuntimeError::new("Invalid TravelInfo set operation")),
        }
    })();
    match result {
        Ok(value) => { unsafe { out.write_unaligned(value); } 0 }
        Err(error) => { if context.error.is_none() { context.error = Some(error); } 1 }
    }
}

/// Performs native TMap::Set: replace a case-insensitive key or append and rehash.
/// # Safety
/// The map is canonical owner storage; key/value must not alias its pair allocation.
pub(crate) unsafe fn insert_travel_map(
    access: CoreAccess<'_>, map: *mut TMapStringString, key: &[u32], value: &[u32],
) -> RuntimeResult<()> {
    if unsafe { (*map).hash_count } == 0 {
        unsafe { (*map).hash_count = 8; travel_map_rehash(map)?; }
    }
    let key = &key[..key.iter().position(|&unit| unit == 0).unwrap_or(key.len())];
    let mut context = TravelMapSet { access, map, key, value, error: None };
    let services = Hp2MapServices {
        context: (&mut context as *mut TravelMapSet<'_>).cast(),
        leaf: travel_map_set_leaf, query: travel_map_set_query,
        allocator: array_storage::system_allocator(), tag: ptr::null(),
    };
    let slots = unsafe { Hp2MapSlots { hash: ptr::addr_of_mut!((*map).hash).cast(), count: ptr::addr_of_mut!((*map).hash_count) } };
    unsafe { map_storage::run(slots, services, map_storage::operation::SET) }
        .map(|_| ())
        .map_err(|code| context.error.take().unwrap_or_else(|| RuntimeError::new(&format!("Unable to set TravelInfo ({code})"))))
}

unsafe fn serialize_travel_map(
    access: CoreAccess<'_>,
    archive: &ObjectArchive<'_>,
    map: *mut TMapStringString,
) -> RuntimeResult<()> {
    if matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
        return Ok(());
    }
    let old_count = unsafe { (*map).pairs.num };
    let old_max = unsafe { (*map).pairs.max };
    if old_count < 0 || old_max < old_count || (old_count > 0 && unsafe { (*map).pairs.data.is_null() }) {
        return Err(RuntimeError::new("Invalid Level TravelInfo pairs"));
    }
    let pair_width = mem::size_of::<FStringMapPair>();
    archive.count_bytes(
        (old_count as usize).checked_mul(pair_width).ok_or_else(|| RuntimeError::new("Level TravelInfo byte count overflow"))?,
        (old_max as usize).checked_mul(pair_width).ok_or_else(|| RuntimeError::new("Level TravelInfo capacity byte count overflow"))?,
    )?;
    let mut count = old_count;
    archive.serialize_compact_index(&mut count)?;
    if !archive.is_loading() {
        let pairs = unsafe { (*map).pairs.data.cast::<FStringMapPair>() };
        for index in 0..old_count as usize {
            let pair = unsafe { pairs.add(index) };
            unsafe {
                access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*pair).key))?;
                access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*pair).value))?;
            }
        }
        return Ok(());
    }
    if count < 0 {
        return Err(RuntimeError::new("Invalid serialized Level TravelInfo count"));
    }
    let mut staged: TMapStringString = unsafe { mem::zeroed() };
    staged.hash_count = 8;
    let slots = array_slots(ptr::addr_of_mut!(staged.pairs));
    let allocator = array_storage::system_allocator();
    if let Err(code) = unsafe {
        array_storage::set(slots, count, pair_width as i32, ptr::null(), allocator)
    } {
        return Err(RuntimeError::new(&format!("Unable to allocate Level TravelInfo pairs ({code})")));
    }
    let pairs = staged.pairs.data.cast::<FStringMapPair>();
    if count > 0 {
        unsafe { ptr::write_bytes(pairs, 0, count as usize); }
    }
    let read_result = (|| {
        for index in 0..count as usize {
            let pair = unsafe { pairs.add(index) };
            unsafe {
                access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*pair).key))?;
                access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*pair).value))?;
            }
        }
        unsafe { travel_map_rehash(&mut staged)?; }
        Ok(())
    })();
    if let Err(error) = read_result {
        let _ = unsafe { destroy_travel_map(access, &mut staged) };
        return Err(error);
    }
    let old = unsafe { ptr::replace(map, staged) };
    let mut old = old;
    unsafe { destroy_travel_map(access, &mut old) }
}
