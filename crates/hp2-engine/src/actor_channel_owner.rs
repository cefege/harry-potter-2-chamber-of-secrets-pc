//! Canonical metadata and Core-owned lifetime for Engine actor channels.

use hp2_abi::engine::generated::{
    AActor, AInventory, APlayerPawn, FActorChannelMapPair, FBitReader, FBitWriter, FClassNetCache,
    FFieldNetCache, FInBunch, FName, FOutBunch, FRepRecord, FRotator, TArrayRaw, UActorChannel,
    UBoolProperty, UChannel, UClass, UField, UFunction, ULevel, UNetConnection, UNetDriver, UObject,
    UPackageMap, UProperty, UStruct,
};
use hp2_abi::FVector;
use hp2_core::array_storage::Hp2ArraySlots;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ffi::{c_void, CStr}, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_TRANSIENT: u32 = 0x0000_0008;

/// Canonical Engine actor-channel class declaration.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "ActorChannel",
    package: "Engine",
    super_name: Some("Channel"),
    within: "Object",
    flags: CLASS_TRANSIENT,
    object_flags: OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UActorChannel>()),
    operations: Some(ACTOR_CHANNEL_OPERATIONS),
    properties: &[],
    defaults: &[],
}];

const ACTOR_CHANNEL_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_actor_channel;
    operations.visit_references = visit_actor_channel;
    operations
};

/// Constructs and initializes an actor channel without publishing it in connection slots.
///
/// `channel_owner::create_channel` owns publication; this function only establishes the
/// fully initialized object that it can safely publish.
///
/// # Safety
/// `connection` must be a live canonical connection with a live driver.
pub unsafe fn create(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
) -> RuntimeResult<*mut UActorChannel> {
    if connection.is_null() {
        return Err(RuntimeError::new("Null canonical UNetConnection"));
    }
    if !(0..1023).contains(&index) {
        return Err(RuntimeError::new("Actor channel index is outside connection slots"));
    }
    let driver = unsafe { (*connection).driver.cast::<UNetDriver>() };
    if driver.is_null() {
        return Err(RuntimeError::new("Actor-channel connection has no driver"));
    }
    let level = unsafe { crate::demo_owner::notify_get_level(access, driver)? };
    let channel = unsafe { construct(access)? };
    if let Err(error) = unsafe { initialize_with_level(access, channel, connection, index, opened_locally, level) } {
        let _ = unsafe { access.conditional_destroy(channel.cast()) };
        let _ = unsafe { access.delete_object(channel.cast()) };
        return Err(error);
    }
    Ok(channel)
}

/// Constructs the canonical Engine.ActorChannel record without initialization or publication.
///
/// # Safety
/// The Core owner must have registered Engine.ActorChannel and its superclass.
pub unsafe fn construct(access: CoreAccess<'_>) -> RuntimeResult<*mut UActorChannel> {
    let class = access.native_class("Engine.ActorChannel")?;
    unsafe {
        access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())
            .map(|object| object.cast())
    }
}

/// Initializes the shared channel base and actor-channel timing/level fields.
///
/// # Safety
/// `channel` and `connection` must be live objects of their declared canonical classes.
pub unsafe fn initialize(
    access: CoreAccess<'_>,
    channel: *mut UActorChannel,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
) -> RuntimeResult<()> {
    if channel.is_null() || connection.is_null() {
        return Err(RuntimeError::new("Invalid actor-channel initialization arguments"));
    }
    let driver = unsafe { (*connection).driver.cast::<UNetDriver>() };
    if driver.is_null() {
        return Err(RuntimeError::new("Actor-channel connection has no driver"));
    }
    let level = unsafe { crate::demo_owner::notify_get_level(access, driver)? };
    unsafe { initialize_with_level(access, channel, connection, index, opened_locally, level) }
}

unsafe fn initialize_with_level(
    _access: CoreAccess<'_>,
    channel: *mut UActorChannel,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
    level: *mut hp2_abi::engine::generated::ULevel,
) -> RuntimeResult<()> {
    let driver = unsafe { (*connection).driver.cast::<UNetDriver>() };
    unsafe {
        crate::channel_owner::init_channel(channel.cast::<UChannel>(), connection, index, opened_locally)?;
        (*channel).level = level.cast();
        (*channel).actor = ptr::null_mut();
        (*channel).actor_class = ptr::null_mut();
        (*channel).relevant_time = (*driver).time;
        (*channel).last_update_time = (f64::from_ne_bytes((*driver).time)
            - f64::from((*driver).spawn_priority_seconds)).to_ne_bytes();
        (*channel).spawn_acked = 0;
        (*channel).num_reps = 0;
    }
    Ok(())
}

/// Deletes a canonical actor channel through the shared channel lifetime path.
///
/// # Safety
/// `channel` must be a live, registered actor channel with intact connection slots.
pub unsafe fn delete(access: CoreAccess<'_>, channel: *mut UActorChannel) -> RuntimeResult<()> {
    if channel.is_null() {
        return Err(RuntimeError::new("Null canonical UActorChannel"));
    }
    unsafe { crate::channel_owner::delete_channel(access, channel.cast()) }
}

#[repr(C)]
struct PropertyRetirement {
    in_packet_id: i32,
    out_packet_id: i32,
    reliable: u8,
    _padding: [u8; 3],
}

/// Handles one negative acknowledgement and marks lost actor properties dirty.
///
/// # Safety
/// `channel` must be live; its connection and actor-channel arrays must remain valid.
pub unsafe fn received_nak(
    access: CoreAccess<'_>,
    channel: *mut UActorChannel,
    packet_id: i32,
) -> RuntimeResult<()> {
    if channel.is_null() {
        return Err(RuntimeError::new("Null canonical UActorChannel"));
    }
    unsafe {
        crate::channel_owner::received_nak(access, channel.cast(), packet_id)?;
        if (*channel).actor_class.is_null() {
            return Ok(());
        }
        let mut index = (*channel).retirement.num;
        while index > 0 {
            index -= 1;
            let retirement = (*channel).retirement.data.cast::<PropertyRetirement>().add(index as usize);
            let value = retirement.read_unaligned();
            if value.out_packet_id == packet_id && value.reliable == 0 {
                let dirty = &mut (*channel).dirty;
                let mut found = false;
                for existing in 0..dirty.num {
                    if dirty.data.cast::<i32>().add(existing as usize).read_unaligned() == index {
                        found = true;
                        break;
                    }
                }
                if !found {
                    let slots = Hp2ArraySlots {
                        data: ptr::addr_of_mut!((*dirty).data).cast(),
                        num: ptr::addr_of_mut!((*dirty).num),
                        max: ptr::addr_of_mut!((*dirty).max),
                    };
                    let added = hp2_core::array_storage::add(
                        slots,
                        1,
                        std::mem::size_of::<i32>() as i32,
                        ptr::null(),
                        hp2_core::array_storage::system_allocator(),
                    ).map_err(|_| RuntimeError::new("Actor-channel dirty list allocation failed"))?;
                    dirty.data.cast::<i32>().add(added as usize).write_unaligned(index);
                }
            }
        }
    }
    Ok(())
}

struct CoreBunchMapping<'a> {
    access: CoreAccess<'a>,
    package_map: *mut hp2_core::core_records::PackageMapRecord,
    bunch: *mut FInBunch,
}

fn name_from_wire(package_map: *mut hp2_core::core_records::PackageMapRecord, wire_index: i32) -> RuntimeResult<i32> {
    if wire_index < 0 { return Err(RuntimeError::new("Negative network name index")); }
    let names = unsafe { ptr::addr_of!((*package_map).name_indices).read_unaligned() };
    if names.num < 0 || (names.num > 0 && names.data.is_null()) {
        return Err(RuntimeError::new("Invalid package-map name indices"));
    }
    for index in 0..names.num {
        if unsafe { names.data.cast::<i32>().add(index as usize).read_unaligned() } == wire_index {
            return Ok(index);
        }
    }
    Err(RuntimeError::new("Network name index is not mapped"))
}

fn name_to_wire(package_map: *mut hp2_core::core_records::PackageMapRecord, global_index: i32) -> RuntimeResult<i32> {
    let names = unsafe { ptr::addr_of!((*package_map).name_indices).read_unaligned() };
    if global_index < 0 || global_index >= names.num || names.data.is_null() {
        return Err(RuntimeError::new("Global name index is outside package-map name indices"));
    }
    Ok(unsafe { names.data.cast::<i32>().add(global_index as usize).read_unaligned() })
}

impl CoreBunchMapping<'_> {
    fn name_from_wire(&self, wire_index: i32) -> RuntimeResult<i32> {
        name_from_wire(self.package_map, wire_index)
    }

    fn name_to_wire(&self, global_index: i32) -> RuntimeResult<i32> {
        name_to_wire(self.package_map, global_index)
    }

    fn reader(&self) -> *mut FBitReader {
        unsafe { ptr::addr_of_mut!((*self.bunch).base).cast() }
    }

    fn check_reader(&self) -> RuntimeResult<()> {
        if unsafe { (*self.reader()).ar_is_error } != 0 {
            Err(RuntimeError::new("Network bit reader overflow"))
        } else {
            Ok(())
        }
    }

    fn read_byte(&self) -> RuntimeResult<u8> {
        let mut byte = 0;
        unsafe { hp2_core::bits::reader_serialize(self.reader(), &mut byte, 1); }
        self.check_reader()?;
        Ok(byte)
    }

}

impl hp2_core::object_archive::ArchiveMapping for CoreBunchMapping<'_> {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject> {
        unsafe { self.access.package_map_index_to_object(self.package_map.cast(), index, true) }
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        unsafe { self.access.package_map_object_to_index(self.package_map.cast(), object) }
    }
    fn load_name(&self, local_index: i32) -> RuntimeResult<i32> { self.name_from_wire(local_index) }
    fn save_name(&self, global_index: i32) -> RuntimeResult<i32> { self.name_to_wire(global_index) }
    fn visit_object(&self, _object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        Err(RuntimeError::new("Network archive cannot visit object references"))
    }
    fn visit_name(&self, _name_index: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new("Network archive cannot visit names"))
    }
}

impl hp2_core::object_archive::NetworkReader for CoreBunchMapping<'_> {
    unsafe fn read_raw(&self, output: *mut u8, count: usize) -> RuntimeResult<()> {
        let count = i32::try_from(count).map_err(|_| RuntimeError::new("Network read length exceeds FBitReader limit"))?;
        unsafe { hp2_core::bits::reader_serialize(self.reader(), output, count); }
        Ok(())
    }
    fn read_bounded(&self, value: &mut u32, maximum: u32) -> RuntimeResult<()> {
        unsafe { hp2_core::bits::reader_serialize_int(self.reader(), value, maximum); }
        Ok(())
    }
    unsafe fn read_bits(&self, output: *mut u8, bit_count: u32) -> RuntimeResult<()> {
        let count = i32::try_from(bit_count).map_err(|_| RuntimeError::new("Network bit count exceeds FBitReader limit"))?;
        unsafe { hp2_core::bits::reader_serialize_bits(self.reader(), output, count); }
        Ok(())
    }
    fn position_bits(&self) -> usize { unsafe { (*self.reader()).pos.max(0) as usize } }
    fn total_bits(&self) -> usize { unsafe { (*self.reader()).num.max(0) as usize } }
    unsafe fn read_object(&self, slot: *mut *mut UObject, expected_class: *mut UClass) -> RuntimeResult<i32> {
        if slot.is_null() { return Err(RuntimeError::new("Null network object destination")); }
        let mut dynamic = 0_u8;
        unsafe { hp2_core::bits::reader_serialize_bits(self.reader(), &mut dynamic, 1); }
        let mut index = 0_u32;
        let object = if unsafe { (*self.reader()).ar_is_error != 0 } {
            ptr::null_mut()
        } else if dynamic != 0 {
            unsafe { hp2_core::bits::reader_serialize_int(self.reader(), &mut index, 1023); }
            let connection = unsafe { (*self.package_map.cast::<hp2_abi::engine::generated::UPackageMapLevel>()).connection.cast::<UNetConnection>() };
            if unsafe { (*self.reader()).ar_is_error == 0 } && index != 0 && index < 1023 && !connection.is_null() {
                let channel = unsafe { (*connection).channels[index as usize] };
                if !channel.is_null() {
                    let base = channel.cast::<UChannel>();
                    if unsafe { (*base).ch_type == 2 && (*base).closing == 0 } {
                        unsafe { (*channel.cast::<UActorChannel>()).actor.cast::<UObject>() }
                    } else { ptr::null_mut() }
                } else { ptr::null_mut() }
            } else { ptr::null_mut() }
        } else {
            let maximum = unsafe { self.access.package_map_max_object_index(self.package_map.cast())? };
            unsafe { hp2_core::bits::reader_serialize_int(self.reader(), &mut index, maximum); }
            if unsafe { (*self.reader()).ar_is_error == 0 } {
                unsafe { self.access.package_map_index_to_object(self.package_map.cast(), index as i32, true)? }
            } else { ptr::null_mut() }
        };
        let object = if !object.is_null() && !expected_class.is_null()
            && !unsafe { hp2_core::object_model::is_a(object.cast(), expected_class.cast()) }
        {
            self.access.emit_log(
                self.access.intern_str("LogNet", hp2_core::name_owner::FindName::Add)?,
                &"Network package map resolved an object of the wrong class".chars().map(u32::from).collect::<Vec<_>>(),
            )?;
            ptr::null_mut()
        } else { object };
        unsafe { slot.write(object); }
        Ok(index as i32)
    }
    unsafe fn read_name(&self, slot: *mut i32) -> RuntimeResult<()> {
        if slot.is_null() { return Err(RuntimeError::new("Null network name destination")); }
        let maximum = unsafe { (*self.package_map).max_name_index.wrapping_add(1) };
        let mut index = 0u32;
        unsafe { hp2_core::bits::reader_serialize_int(self.reader(), &mut index, maximum); }
        if unsafe { (*self.reader()).ar_is_error } == 0 {
            let global = if index == unsafe { (*self.package_map).max_name_index } {
                0
            } else {
                self.name_from_wire(index as i32)?
            };
            unsafe { slot.write(global); }
        }
        Ok(())
    }
}

/// Borrows the actual bunch cursor and canonical package mapping without copying
/// packet bytes or fabricating a native FArchive.
///
/// # Safety
/// Bunch, connection and package map remain live through `body`. The callback
/// cannot retain this archive or recursively mutate its packet cursor.
pub(crate) unsafe fn with_core_bunch_archive<T>(
    access: CoreAccess<'_>, bunch: *mut FInBunch,
    body: impl FnOnce(&hp2_core::object_archive::ObjectArchive<'_>) -> RuntimeResult<T>,
) -> RuntimeResult<T> {
    let connection = unsafe { (*bunch).connection.cast::<UNetConnection>() };
    let mapping = CoreBunchMapping {
        access, package_map: unsafe { (*connection).package_map.cast() }, bunch,
    };
    let reader = mapping.reader();
    let flags = hp2_core::object_archive::ArchiveFlags::new(
        unsafe { (*reader).ar_ver as u16 }, unsafe { (*reader).ar_licensee_ver as u16 },
        unsafe { (*reader).ar_is_persistent != 0 },
    );
    let archive = hp2_core::object_archive::ObjectArchive::network_loading(&mapping, flags, &mapping);
    body(&archive)
}
unsafe fn next_property_field(mut field: *mut UField, property_class: *mut UClass) -> *mut UProperty {
    unsafe {
        while !field.is_null() && !hp2_core::object_model::is_a(field.cast(), property_class.cast()) {
            field = (*field).next.cast();
        }
        field.cast()
    }
}

struct CoreActorOps<'a> {
    access: CoreAccess<'a>,
}

impl CoreActorOps<'_> {
    fn log_message(&self, message: &str) -> RuntimeResult<()> {
        let category = self.access.intern_str("LogNet", hp2_core::name_owner::FindName::Add)?;
        let units = message.chars().map(u32::from).collect::<Vec<_>>();
        self.access.emit_log(category, &units)
    }

    unsafe fn actor_map(&self, connection: *mut UNetConnection) -> RuntimeResult<*mut TArrayRaw> {
        if connection.is_null() { return Err(RuntimeError::new("Null actor-channel connection")); }
        Ok(unsafe { ptr::addr_of_mut!((*connection).actor_channels.pairs) })
    }

    unsafe fn is_class(&self, object: *mut c_void, class_name: &str) -> RuntimeResult<bool> {
        if object.is_null() { return Ok(false); }
        let target = self.access.native_class(class_name)?;
        Ok(unsafe { hp2_core::object_model::is_a(object, target.cast()) })
    }

    unsafe fn append_zeroed(&self, array: *mut TArrayRaw, count: i32, width: i32) -> RuntimeResult<()> {
        if count < 0 { return Err(RuntimeError::new("Negative actor-channel array count")); }
        if count == 0 { return Ok(()); }
        let slots = Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*array).data).cast() },
            num: unsafe { ptr::addr_of_mut!((*array).num) },
            max: unsafe { ptr::addr_of_mut!((*array).max) },
        };
        unsafe { hp2_core::array_storage::add_zeroed(slots, count, width, ptr::null(), hp2_core::array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Actor-channel array allocation failed"))?;
        Ok(())
    }
}

impl crate::net_chan::ActorSetActorOps for CoreActorOps<'_> {
    type Error = RuntimeError;

    unsafe fn check_fail(&mut self, expression: &'static CStr, line: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(&format!(
            "ActorChannel invariant failed at net_chan.cpp:{line}: {}",
            expression.to_string_lossy()
        )))
    }

    unsafe fn actor_class(&mut self, actor: *mut c_void) -> RuntimeResult<*mut UClass> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor in actor-channel assignment")); }
        Ok(unsafe { (*actor.cast::<UObject>()).class.cast() })
    }

    unsafe fn class_net_cache(&mut self, map: *mut UPackageMap, class: *mut UClass) -> RuntimeResult<*mut FClassNetCache> {
        unsafe { self.access.package_map_get_class_net_cache(map, class) }
    }

    unsafe fn actor_channel_set(
        &mut self,
        connection: *mut UNetConnection,
        actor: *mut c_void,
        channel: *mut UActorChannel,
    ) -> RuntimeResult<()> {
        unsafe { set_actor_channel_map(connection, actor, channel) }
    }

    unsafe fn rep_eval_add_zeroed(&mut self, channel: *mut UActorChannel, count: i32) -> RuntimeResult<()> {
        unsafe { self.append_zeroed(ptr::addr_of_mut!((*channel).rep_eval), count, 4) }
    }

    unsafe fn actor_flags(&mut self, actor: *mut c_void) -> RuntimeResult<(i32, i32, i32, i32)> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor flags query")); }
        let object = actor.cast::<UObject>();
        let index = unsafe { (*object).index };
        let valid = index >= 0 && index < self.access.object_count() && self.access.object_at(index) == object;
        let actor = actor.cast::<AActor>();
        Ok((
            i32::from(valid),
            i32::from(unsafe { (*actor).b_net_temporary() }),
            i32::from(unsafe { (*actor).b_static() }),
            i32::from(unsafe { (*actor).b_no_delete() }),
        ))
    }

    unsafe fn recent_add(&mut self, channel: *mut UActorChannel, size: i32) -> RuntimeResult<*mut u8> {
        if size < 0 { return Err(RuntimeError::new("Negative actor-channel recent size")); }
        let old_num = unsafe { (*channel).recent.num };
        let slots = Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*channel).recent.data).cast() },
            num: unsafe { ptr::addr_of_mut!((*channel).recent.num) },
            max: unsafe { ptr::addr_of_mut!((*channel).recent.max) },
        };
        let start = unsafe { hp2_core::array_storage::add_zeroed(slots, size, 1, ptr::null(), hp2_core::array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Actor-channel recent data allocation failed"))?;
        if start != old_num { return Err(RuntimeError::new("Actor-channel recent buffer was not appended")); }
        Ok(unsafe { (*channel).recent.data.cast::<u8>().add(start as usize) })
    }

    unsafe fn object_init_properties(&mut self, data: *mut u8, size: i32, class: *mut UClass) -> RuntimeResult<()> {
        if data.is_null() || class.is_null() || size < 0 { return Err(RuntimeError::new("Invalid recent property initialization")); }
        let defaults = unsafe { (*class).defaults };
        if defaults.num != size || (size > 0 && defaults.data.is_null()) {
            return Err(RuntimeError::new("Actor defaults do not match recent property storage"));
        }
        let mut property = unsafe { (*class.cast::<UStruct>()).constructor_link };
        while !property.is_null() {
            let meta = property.cast::<UProperty>();
            let offset = unsafe { (*meta).offset };
            let element_size = unsafe { (*meta).element_size };
            let dim = unsafe { (*meta).array_dim };
            let end = offset.checked_add(element_size.checked_mul(dim).ok_or_else(|| RuntimeError::new("Recent property size overflow"))?)
                .ok_or_else(|| RuntimeError::new("Recent property range overflow"))?;
            if offset < 0 || end > size { return Err(RuntimeError::new("Actor recent property exceeds class defaults")); }
            let dest = unsafe { data.offset(offset as isize).cast() };
            let source = unsafe { defaults.data.cast::<u8>().offset(offset as isize).cast() };
            let status = unsafe {
                self.access.property_value(
                    hp2_core::property_value::Hp2ValueOperation::ValueCopyComplete,
                    meta,
                    dest,
                    source,
                    0,
                    None,
                )?
            };
            if status == 0 { return Err(RuntimeError::new("Actor default property copy failed")); }
            property = unsafe { (*meta).constructor_link_next.cast() };
        }
        Ok(())
    }

    unsafe fn property_metadata(&mut self, property: *mut UProperty) -> RuntimeResult<crate::net_chan::ActorPropertyMetadata> {
        if property.is_null() { return Err(RuntimeError::new("Null replicated property")); }
        Ok(crate::net_chan::ActorPropertyMetadata {
            offset: unsafe { (*property).offset },
            element_size: unsafe { (*property).element_size },
            array_dim: unsafe { (*property).array_dim },
            index: i32::from(unsafe { (*property).rep_index }),
            flags: unsafe { (*property).property_flags as i32 },
        })
    }

    unsafe fn property_bool_mask(&mut self, property: *mut UProperty) -> RuntimeResult<u32> {
        if !unsafe { self.is_class(property.cast(), "Core.BoolProperty")? } {
            return Err(RuntimeError::new("Bool mask requested for a non-bool property"));
        }
        Ok(unsafe { (*property.cast::<UBoolProperty>()).bit_mask })
    }

    unsafe fn property_is_bool(&mut self, property: *mut UProperty) -> RuntimeResult<bool> {
        unsafe { self.is_class(property.cast(), "Core.BoolProperty") }
    }

    unsafe fn property_destroy_value(&mut self, property: *mut UProperty, data: *mut c_void) -> RuntimeResult<()> {
        let status = unsafe { self.access.property_value(hp2_core::property_value::Hp2ValueOperation::ValueDestroy, property, data, ptr::null_mut(), 0, None)? };
        if status == 0 { Err(RuntimeError::new("Actor-channel property destruction failed")) } else { Ok(()) }
    }

    unsafe fn mem_zero(&mut self, data: *mut u8, size: i32) -> RuntimeResult<()> {
        if size < 0 || (size > 0 && data.is_null()) { return Err(RuntimeError::new("Invalid actor-channel zero range")); }
        if size > 0 { unsafe { ptr::write_bytes(data, 0, size as usize); } }
        Ok(())
    }

    unsafe fn retirement_empty(&mut self, channel: *mut UActorChannel, count: i32) -> RuntimeResult<()> {
        if count < 0 { return Err(RuntimeError::new("Negative retirement count")); }
        let slots = Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*channel).retirement.data).cast() },
            num: unsafe { ptr::addr_of_mut!((*channel).retirement.num) },
            max: unsafe { ptr::addr_of_mut!((*channel).retirement.max) },
        };
        unsafe { hp2_core::array_storage::empty(slots, count, std::mem::size_of::<PropertyRetirement>() as i32, ptr::null(), hp2_core::array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Retirement array allocation failed"))?;
        Ok(())
    }

    unsafe fn retirement_num(&mut self, channel: *mut UActorChannel) -> RuntimeResult<i32> {
        Ok(unsafe { (*channel).retirement.num })
    }

    unsafe fn retirement_add(&mut self, channel: *mut UActorChannel) -> RuntimeResult<()> {
        unsafe { self.append_zeroed(ptr::addr_of_mut!((*channel).retirement), 1, std::mem::size_of::<PropertyRetirement>() as i32) }
    }
}

unsafe fn set_actor_channel_map(
    connection: *mut UNetConnection,
    actor: *mut c_void,
    channel: *mut UActorChannel,
) -> RuntimeResult<()> {
    if connection.is_null() || actor.is_null() || channel.is_null() {
        return Err(RuntimeError::new("Invalid ActorChannels map insertion"));
    }
    let map = unsafe { ptr::addr_of_mut!((*connection).actor_channels) };
    let array = unsafe { ptr::addr_of_mut!((*map).pairs) };
    let count = unsafe { (*map).hash_count };
    let hash = unsafe { (*map).hash.cast::<i32>() };
    if count <= 0 || !(count as u32).is_power_of_two() || hash.is_null() {
        return Err(RuntimeError::new("Invalid ActorChannels hash table"));
    }
    let pairs = unsafe { (*array).data.cast::<FActorChannelMapPair>() };
    let mut found = None;
    for index in 0..unsafe { (*array).num } {
        let pair = unsafe { pairs.add(index as usize) };
        if unsafe { (*pair).key == actor } { found = Some(index); break; }
    }
    let pair_index = if let Some(index) = found {
        index
    } else {
        let slots = Hp2ArraySlots {
            data: unsafe { ptr::addr_of_mut!((*array).data).cast() },
            num: unsafe { ptr::addr_of_mut!((*array).num) },
            max: unsafe { ptr::addr_of_mut!((*array).max) },
        };
        unsafe { hp2_core::array_storage::add(slots, 1, std::mem::size_of::<FActorChannelMapPair>() as i32, ptr::null(), hp2_core::array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("ActorChannels pair allocation failed"))?
    };
    let pairs = unsafe { (*array).data.cast::<FActorChannelMapPair>() };
    unsafe {
        (*pairs.add(pair_index as usize)).key = actor;
        (*pairs.add(pair_index as usize)).value = channel.cast();
        for bucket in 0..count { hash.add(bucket as usize).write_unaligned(-1); }
        for index in 0..(*array).num {
            let pair = pairs.add(index as usize);
            let key = (*pair).key;
            if key.is_null() { return Err(RuntimeError::new("Null ActorChannels key")); }
            let bucket = ((*key.cast::<UObject>()).index & (count - 1)) as usize;
            (*pair).hash_next = hash.add(bucket).read_unaligned();
            hash.add(bucket).write_unaligned(index);
        }
    }
    Ok(())
}
impl crate::net_chan::ActorReceiveOps for CoreActorOps<'_> {
    unsafe fn invalid_new_channel_packet(&mut self, bunch: *mut FInBunch) -> RuntimeResult<()> {
        self.log_message(&format!(
            "Invalid new ActorChannel packet (open={}, close={}, reliable={})",
            unsafe { (*bunch).b_open }, unsafe { (*bunch).b_close }, unsafe { (*bunch).b_reliable }
        ))
    }

    unsafe fn serialize_object(&mut self, bunch: *mut FInBunch) -> RuntimeResult<*mut c_void> {
        let connection = unsafe { (*bunch).connection.cast::<UNetConnection>() };
        if connection.is_null() { return Err(RuntimeError::new("Actor bunch has no connection")); }
        let mapping = CoreBunchMapping { access: self.access, package_map: unsafe { (*connection).package_map.cast() }, bunch };
        let mut object = ptr::null_mut::<UObject>();
        unsafe { hp2_core::object_archive::NetworkReader::read_object(&mapping, &mut object, ptr::null_mut())?; }
        Ok(object.cast())
    }

    unsafe fn object_is_actor(&mut self, object: *mut c_void) -> RuntimeResult<bool> {
        unsafe { self.is_class(object, "Engine.Actor") }
    }
    unsafe fn object_is_class(&mut self, object: *mut c_void) -> RuntimeResult<bool> {
        unsafe { self.is_class(object, "Core.Class") }
    }
    unsafe fn class_is_actor_subclass(&mut self, class: *mut c_void) -> RuntimeResult<bool> {
        if class.is_null() { return Ok(false); }
        let actor_class = self.access.native_class("Engine.Actor")?;
        let mut current = class.cast::<UClass>();
        while !current.is_null() {
            if current == actor_class { return Ok(true); }
            current = unsafe { (*current).super_field.cast() };
        }
        Ok(false)
    }
    unsafe fn log_invalid_actor_class(&mut self) -> RuntimeResult<()> {
        self.log_message("Invalid actor class in ActorChannel bunch")
    }
    unsafe fn serialize_vector(&mut self, bunch: *mut FInBunch) -> RuntimeResult<FVector> {
        let mut vector = FVector { x: 0.0, y: 0.0, z: 0.0 };
        let connection = unsafe { (*bunch).connection.cast::<UNetConnection>() };
        let mapping = CoreBunchMapping { access: self.access, package_map: unsafe { (*connection).package_map.cast() }, bunch };
        unsafe {
            hp2_core::object_archive::NetworkReader::read_raw(
                &mapping,
                (&mut vector as *mut FVector).cast(),
                std::mem::size_of::<FVector>(),
            )?;
        }
        Ok(vector)
    }
    unsafe fn spawn_actor(&mut self, level: *mut c_void, class: *mut c_void, location: FVector) -> RuntimeResult<*mut c_void> {
        let actor = unsafe {
            crate::level_spawn_actor_owner::spawn_actor(
                self.access,
                level.cast::<ULevel>(),
                class.cast::<UClass>(),
                FName { index: 0 },
                ptr::null_mut(),
                ptr::null_mut(),
                location,
                FRotator { pitch: 0, yaw: 0, roll: 0 },
                ptr::null_mut(),
                true,
                true,
            )?
        };
        Ok(actor.cast())
    }
    unsafe fn log_actor_not_spawned(&mut self) -> RuntimeResult<()> {
        self.log_message("ActorChannel actor spawn failed")
    }
    unsafe fn actor_pre_net_receive(&mut self, actor: *mut c_void) -> RuntimeResult<()> {
        let _ = unsafe { self.access.execute_named_event(actor.cast(), "PreNetReceive")? };
        Ok(())
    }
    unsafe fn actor_post_net_receive(&mut self, actor: *mut c_void) -> RuntimeResult<()> {
        let _ = unsafe { self.access.execute_named_event(actor.cast(), "PostNetReceive")? };
        Ok(())
    }
    unsafe fn actor_post_net_begin_play(&mut self, actor: *mut c_void) -> RuntimeResult<()> {
        let _ = unsafe { self.access.execute_named_event(actor.cast(), "PostNetBeginPlay")? };
        Ok(())
    }
    unsafe fn actor_is_player_pawn(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        unsafe { self.is_class(actor, "Engine.PlayerPawn") }
    }
    unsafe fn actor_player(&mut self, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        if !unsafe { self.is_class(actor, "Engine.PlayerPawn")? } { return Ok(ptr::null_mut()); }
        Ok(unsafe { (*actor.cast::<APlayerPawn>()).player })
    }
    unsafe fn player_is_viewport(&mut self, player: *mut c_void) -> RuntimeResult<bool> {
        unsafe { self.is_class(player, "Engine.Viewport") }
    }
    unsafe fn cache_max_index(&mut self, cache: *mut FClassNetCache) -> RuntimeResult<i32> {
        if cache.is_null() { return Err(RuntimeError::new("Null class replication cache")); }
        Ok(unsafe { (*cache).fields_base.wrapping_add((*cache).fields.num) })
    }
    unsafe fn bunch_read_int(&mut self, bunch: *mut FInBunch, max_index: i32) -> RuntimeResult<i32> {
        if max_index < 0 { return Err(RuntimeError::new("Negative class cache maximum index")); }
        let connection = unsafe { (*bunch).connection.cast::<UNetConnection>() };
        let mapping = CoreBunchMapping { access: self.access, package_map: unsafe { (*connection).package_map.cast() }, bunch };
        let mut value = 0u32;
        hp2_core::object_archive::NetworkReader::read_bounded(&mapping, &mut value, max_index as u32)?;
        Ok(value as i32)
    }
    unsafe fn bunch_is_error(&mut self, bunch: *mut FInBunch) -> RuntimeResult<bool> {
        Ok(unsafe { (*bunch).base.ar_is_error != 0 })
    }
    unsafe fn cache_from_index(&mut self, cache: *mut FClassNetCache, index: i32) -> RuntimeResult<*mut FFieldNetCache> {
        if cache.is_null() || index < 0 { return Ok(ptr::null_mut()); }
        let mut current = cache;
        while !current.is_null() {
            let base = unsafe { (*current).fields_base };
            let fields = unsafe { (*current).fields };
            if index >= base && index < base.saturating_add(fields.num) {
                let local = index - base;
                if fields.data.is_null() { return Err(RuntimeError::new("Class cache fields are missing")); }
                return Ok(unsafe { fields.data.cast::<FFieldNetCache>().add(local as usize) });
            }
            current = unsafe { (*current).super_.cast() };
        }
        Ok(ptr::null_mut())
    }
    unsafe fn field_kind(&mut self, field: *mut c_void) -> RuntimeResult<i32> {
        if unsafe { self.is_class(field, "Core.Property")? } { return Ok(1); }
        if unsafe { self.is_class(field, "Core.Function")? } { return Ok(2); }
        Ok(0)
    }
    unsafe fn property_owner_class(&mut self, property: *mut UProperty) -> RuntimeResult<*mut UClass> {
        if property.is_null() { return Err(RuntimeError::new("Null property owner lookup")); }
        let mut outer = unsafe { (*property.cast::<UObject>()).outer };
        while !outer.is_null() {
            if unsafe { hp2_core::object_model::is_a(outer, self.access.native_class("Core.Class")?.cast()) } {
                return Ok(outer.cast());
            }
            outer = unsafe { (*outer.cast::<UObject>()).outer };
        }
        Err(RuntimeError::new("Replicated property has no owning class"))
    }
    unsafe fn actor_roles(&mut self, actor: *mut c_void) -> RuntimeResult<(u8, u8)> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor role lookup")); }
        Ok(unsafe { ((*actor.cast::<AActor>()).role, (*actor.cast::<AActor>()).remote_role) })
    }
    unsafe fn frame_step_condition(&mut self, actor: *mut c_void, owner_class: *mut UClass, rep_offset: i32) -> RuntimeResult<u32> {
        unsafe { self.access.eval_rep_condition(actor.cast(), owner_class, rep_offset, ptr::null_mut()) }
    }
    unsafe fn retirement_get(&mut self, channel: *mut UActorChannel, index: i32) -> RuntimeResult<*mut c_void> {
        let retirement = unsafe { (*channel).retirement };
        if index < 0 || index >= retirement.num || retirement.data.is_null() {
            return Err(RuntimeError::new("Retirement index is outside actor channel storage"));
        }
        Ok(unsafe { retirement.data.cast::<PropertyRetirement>().add(index as usize).cast() })
    }
    unsafe fn retirement_in_packet(&mut self, retirement: *mut c_void) -> RuntimeResult<i32> {
        if retirement.is_null() { return Err(RuntimeError::new("Null retirement entry")); }
        Ok(unsafe { (*retirement.cast::<PropertyRetirement>()).in_packet_id })
    }
    unsafe fn retirement_set_in_packet(&mut self, retirement: *mut c_void, packet: i32) -> RuntimeResult<()> {
        if retirement.is_null() { return Err(RuntimeError::new("Null retirement entry")); }
        unsafe { (*retirement.cast::<PropertyRetirement>()).in_packet_id = packet; }
        Ok(())
    }
    unsafe fn with_network_mem_scope<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let memory = self.access.scratch_memory();
        let mark = unsafe { memory.mark() };
        let result = body(self);
        unsafe { memory.pop(&mark); }
        result
    }
    unsafe fn mem_new_zeroed(&mut self, size: i32) -> RuntimeResult<*mut u8> {
        if size < 0 { return Err(RuntimeError::new("Negative network scratch allocation")); }
        let pointer = unsafe {
            self.access.scratch_memory().push_bytes(size, std::mem::align_of::<usize>() as i32)?
        };
        unsafe { ptr::write_bytes(pointer, 0, size as usize); }
        Ok(pointer)
    }
    unsafe fn property_net_serialize(&mut self, property: *mut UProperty, bunch: *mut FInBunch, package_map: *mut UPackageMap, data: *mut u8) -> RuntimeResult<()> {
        let mapping = CoreBunchMapping { access: self.access, package_map: package_map.cast(), bunch };
        let reader = mapping.reader();
        let flags = hp2_core::object_archive::ArchiveFlags::new(
            unsafe { (*reader).ar_ver as u16 },
            unsafe { (*reader).ar_licensee_ver as u16 },
            unsafe { (*reader).ar_is_persistent != 0 },
        );
        let archive = hp2_core::object_archive::ObjectArchive::network_loading(&mapping, flags, &mapping);
        // Native receive ignores the mapping boolean; an unresolved object is
        // not a C++ exception. The common policy inspects the reader error flag.
        unsafe {
            self.access.property_net_serialize(property, package_map.cast(), &archive, data.cast(), ptr::null_mut(), 0)?;
        }
        Ok(())
    }
    unsafe fn property_copy_single(&mut self, property: *mut UProperty, destination: *mut u8, source: *mut u8) -> RuntimeResult<()> {
        let status = unsafe {
            self.access.property_value(
                hp2_core::property_value::Hp2ValueOperation::ValueCopySingle,
                property,
                destination.cast(),
                source.cast(),
                0,
                None,
            )?
        };
        if status == 0 { Err(RuntimeError::new("Core network property copy failed")) } else { Ok(()) }
    }
    unsafe fn bunch_serialize_byte(&mut self, bunch: *mut FInBunch) -> RuntimeResult<u8> {
        let connection = unsafe { (*bunch).connection.cast::<UNetConnection>() };
        let mapping = CoreBunchMapping { access: self.access, package_map: unsafe { (*connection).package_map.cast() }, bunch };
        mapping.read_byte()
    }
    unsafe fn field_name(&mut self, field: *mut c_void) -> RuntimeResult<i32> {
        if field.is_null() { return Err(RuntimeError::new("Null replicated field name")); }
        Ok(i32::from_ne_bytes(unsafe { (*field.cast::<UObject>()).name }))
    }
    unsafe fn function_find(&mut self, actor: *mut c_void, name: i32) -> RuntimeResult<*mut UFunction> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor function lookup")); }
        unsafe { self.access.find_object_function_by_name(actor.cast(), name, false) }
    }
    unsafe fn function_super(&mut self, function: *mut UFunction) -> RuntimeResult<*mut UFunction> {
        if function.is_null() { return Err(RuntimeError::new("Null function superclass lookup")); }
        Ok(unsafe { (*function.cast::<hp2_abi::engine::generated::UField>()).super_field.cast() })
    }
    unsafe fn function_metadata(&mut self, function: *mut UFunction) -> RuntimeResult<(*mut UClass, i32)> {
        if function.is_null() { return Err(RuntimeError::new("Null function metadata")); }
        let mut outer = unsafe { (*function.cast::<UObject>()).outer };
        while !outer.is_null() {
            if unsafe { hp2_core::object_model::is_a(outer, self.access.native_class("Core.Class")?.cast()) } {
                return Ok((outer.cast(), i32::from(unsafe { (*function).parms_size })));
            }
            outer = unsafe { (*outer.cast::<UObject>()).outer };
        }
        Err(RuntimeError::new("Function has no owning class"))
    }
    unsafe fn log_received_unwanted_function(&mut self, name: i32, _actor: *mut c_void) -> RuntimeResult<()> {
        let name = self.access.name_text(name)?;
        let text = name.into_iter().filter_map(char::from_u32).collect::<String>();
        self.log_message(&format!("Rejected unwanted network function {text}"))
    }
    unsafe fn function_param_first(&mut self, function: *mut UFunction) -> RuntimeResult<*mut UProperty> {
        if function.is_null() { return Err(RuntimeError::new("Null function parameter lookup")); }
        let property_class = self.access.native_class("Core.Property")?;
        Ok(unsafe { next_property_field((*function.cast::<UStruct>()).children.cast(), property_class) })
    }
    unsafe fn function_param_next(&mut self, parameter: *mut UProperty) -> RuntimeResult<*mut UProperty> {
        if parameter.is_null() { return Ok(ptr::null_mut()); }
        let property_class = self.access.native_class("Core.Property")?;
        Ok(unsafe { next_property_field((*parameter.cast::<UField>()).next.cast(), property_class) })
    }
    unsafe fn object_to_index(&mut self, package_map: *mut UPackageMap, object: *mut c_void) -> RuntimeResult<i32> {
        unsafe { self.access.package_map_object_to_index(package_map, object.cast()) }
    }
    unsafe fn bunch_read_bit(&mut self, bunch: *mut FInBunch) -> RuntimeResult<bool> {
        let connection = unsafe { (*bunch).connection.cast::<UNetConnection>() };
        let mapping = CoreBunchMapping { access: self.access, package_map: unsafe { (*connection).package_map.cast() }, bunch };
        let mut value = 0u8;
        unsafe { hp2_core::object_archive::NetworkReader::read_bits(&mapping, &mut value, 1)?; }
        Ok(value != 0)
    }
    unsafe fn actor_set_client_demo_net_func(&mut self, actor: *mut c_void, value: bool) -> RuntimeResult<()> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor demo RPC flag")); }
        unsafe { (*actor.cast::<AActor>()).set_b_client_demo_net_func(value); }
        Ok(())
    }
    unsafe fn actor_process_event(&mut self, actor: *mut c_void, function: *mut UFunction, parameters: *mut u8) -> RuntimeResult<()> {
        unsafe { crate::actor_owner::process_event(self.access, actor.cast(), function, parameters.cast(), ptr::null_mut()) }
    }
    unsafe fn actor_handle_client_player(&mut self, connection: *mut UNetConnection, actor: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            crate::connection_player_owner::handle_client_player(
                self.access,
                connection,
                actor.cast::<APlayerPawn>(),
            )
        }
    }
}
/// Processes one received actor-channel bunch using the canonical actor replication policy.
///
/// # Safety
/// `channel` and `bunch` must be live objects, and their connection/package map must remain valid.
pub unsafe fn received_bunch(
    access: CoreAccess<'_>,
    channel: *mut UActorChannel,
    bunch: *mut FInBunch,
) -> RuntimeResult<()> {
    if channel.is_null() || bunch.is_null() {
        return Err(RuntimeError::new("Invalid actor-channel bunch arguments"));
    }
    unsafe {
        crate::net_chan::actor_received_bunch_with(
            channel, bunch, &mut CoreActorOps { access },
        )
    }
}

/// Visits the UObject references owned by UActorChannel and its UChannel base.
///
/// # Safety
/// `object` must be a live canonical actor channel and `archive` must remain valid.

const MAX_CHANNELS: u32 = 1023;

fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*array).data).cast() },
        num: unsafe { ptr::addr_of_mut!((*array).num) },
        max: unsafe { ptr::addr_of_mut!((*array).max) },
    }
}

fn bit_error(expression: &[u8], line: i32) -> RuntimeResult<()> {
    let expression = CStr::from_bytes_with_nul(expression)
        .map_err(|_| RuntimeError::new("Invalid bit-writer assertion expression"))?;
    Err(RuntimeError::new(&format!(
        "{} (UnBits.cpp:{line})",
        expression.to_string_lossy()
    )))
}

fn writer_of(bunch: *mut FOutBunch) -> *mut FBitWriter {
    unsafe { ptr::addr_of_mut!((*bunch).base) }
}

struct CoreOutBunchMapping<'a> {
    access: CoreAccess<'a>,
    package_map: *mut hp2_core::core_records::PackageMapRecord,
}

impl hp2_core::object_archive::ArchiveMapping for CoreOutBunchMapping<'_> {
    fn load_object(&self, _index: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new("Saving network archive cannot resolve objects"))
    }
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32> {
        unsafe { self.access.package_map_object_to_index(self.package_map.cast(), object) }
    }
    fn load_name(&self, _local_index: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new("Saving network archive cannot resolve names"))
    }
    fn save_name(&self, global_index: i32) -> RuntimeResult<i32> {
        name_to_wire(self.package_map, global_index)
    }
    fn visit_object(&self, _object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        Err(RuntimeError::new("Network archive cannot visit object references"))
    }
    fn visit_name(&self, _name_index: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new("Network archive cannot visit names"))
    }
}

struct CoreOutBunchWriter<'a> {
    access: CoreAccess<'a>,
    package_map: *mut hp2_core::core_records::PackageMapRecord,
    bunch: *mut FOutBunch,
}

impl CoreOutBunchWriter<'_> {
    unsafe fn write_int(&self, value: u32, maximum: u32) -> RuntimeResult<()> {
        unsafe {
            hp2_core::bits::writer_write_int_with_checker(
                writer_of(self.bunch),
                value,
                maximum,
                hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks,
                bit_error,
            )
        }
    }

    unsafe fn write_bit(&self, value: u8) {
        unsafe { hp2_core::bits::writer_write_bit(writer_of(self.bunch), value); }
    }

    unsafe fn write_bytes(&self, data: *const u8, count: usize) -> RuntimeResult<()> {
        let count = i32::try_from(count).map_err(|_| RuntimeError::new("Network write length exceeds FBitWriter limit"))?;
        unsafe { hp2_core::bits::writer_serialize(writer_of(self.bunch), data, count); }
        Ok(())
    }
}

impl hp2_core::object_archive::NetworkWriter for CoreOutBunchWriter<'_> {
    unsafe fn write_raw(&self, input: *const u8, count: usize) -> RuntimeResult<()> {
        unsafe { self.write_bytes(input, count) }
    }

    fn write_bounded(&self, value: u32, maximum: u32) -> RuntimeResult<()> {
        unsafe { self.write_int(value, maximum) }
    }

    unsafe fn write_bits(&self, input: *const u8, bit_count: u32) -> RuntimeResult<()> {
        let count = i32::try_from(bit_count).map_err(|_| RuntimeError::new("Network bit count exceeds FBitWriter limit"))?;
        unsafe { hp2_core::bits::writer_serialize_bits(writer_of(self.bunch), input, count); }
        Ok(())
    }

    fn position_bits(&self) -> usize { unsafe { (*writer_of(self.bunch)).num.max(0) as usize } }
    fn total_bits(&self) -> usize { unsafe { (*writer_of(self.bunch)).max.max(0) as usize } }

    unsafe fn write_object(&self, slot: *mut *mut UObject, _expected_class: *mut UClass) -> RuntimeResult<i32> {
        if slot.is_null() { return Err(RuntimeError::new("Null network object source")); }
        let object = unsafe { slot.read_unaligned() };
        let map = self.package_map.cast::<hp2_abi::engine::generated::UPackageMapLevel>();
        let connection = unsafe { (*map).connection.cast::<UNetConnection>() };
        let actor_class = self.access.native_class("Engine.Actor")?;
        let actor = object.cast::<AActor>();
        if !object.is_null()
            && unsafe { hp2_core::object_model::is_a(object.cast(), actor_class.cast()) }
            && !unsafe { (*actor).b_static() }
            && !unsafe { (*actor).b_no_delete() }
        {
            unsafe { self.write_bit(1); }
            let channel = unsafe { crate::network_owner::actor_channel(connection, actor)? };
            let (index, mapped) = if channel.is_null() {
                (0, false)
            } else {
                (unsafe { (*channel).ch_index } as u32, unsafe { (*channel).open_acked } != 0)
            };
            unsafe { self.write_int(index, MAX_CHANNELS)?; }
            return Ok(i32::from(mapped));
        }
        let mut index = 0u32;
        if !object.is_null() {
            index = unsafe { self.access.package_map_object_to_index(self.package_map.cast(), object)? } as u32;
        }
        if object.is_null() || index == u32::MAX {
            unsafe {
                self.write_bit(1);
                self.write_int(index, MAX_CHANNELS)?;
            }
        } else {
            let maximum = unsafe { self.access.package_map_max_object_index(self.package_map.cast())? };
            unsafe {
                self.write_bit(0);
                self.write_int(index, maximum)?;
            }
        }
        Ok(1)
    }

    unsafe fn write_name(&self, slot: *mut i32) -> RuntimeResult<()> {
        if slot.is_null() { return Err(RuntimeError::new("Null network name source")); }
        let global = unsafe { slot.read_unaligned() };
        let names = unsafe { ptr::addr_of!((*self.package_map).name_indices).read_unaligned() };
        let wire = if global >= 0 && global < names.num && !names.data.is_null() {
            unsafe { names.data.cast::<i32>().add(global as usize).read_unaligned() }
        } else {
            unsafe { (*self.package_map).max_name_index as i32 }
        };
        unsafe {
            self.write_int(
                wire as u32,
                (*self.package_map).max_name_index.wrapping_add(1),
            )
        }
    }
}

unsafe fn can_serialize_object_core(
    access: CoreAccess<'_>,
    map: *mut hp2_abi::engine::generated::UPackageMap,
    object: *mut c_void,
) -> RuntimeResult<bool> {
    if object.is_null() { return Ok(true); }
    let actor_class = access.native_class("Engine.Actor")?;
    if !unsafe { hp2_core::object_model::is_a(object, actor_class.cast()) } {
        return Ok(true);
    }
    let actor = object.cast::<AActor>();
    if unsafe { (*actor).b_static() || (*actor).b_no_delete() } {
        return Ok(true);
    }
    let map = map.cast::<hp2_abi::engine::generated::UPackageMapLevel>();
    let connection = unsafe { (*map).connection.cast::<UNetConnection>() };
    Ok(!unsafe { crate::network_owner::actor_channel(connection, actor)? }.is_null())
}

struct CoreRepBackend<'a> {
    access: CoreAccess<'a>,
    error: std::cell::RefCell<Option<RuntimeError>>,
}

impl CoreRepBackend<'_> {
    fn fail<T>(&self, error: RuntimeError) -> crate::actor_rep::RepResult<T> {
        *self.error.borrow_mut() = Some(error);
        Err(-1)
    }

    fn take_error(&self) -> RuntimeError {
        self.error.borrow_mut().take()
            .unwrap_or_else(|| RuntimeError::new("Canonical actor replication failed"))
    }
}

fn rep_class_name(kind: u32) -> Option<&'static str> {
    Some(match kind {
        0 => "Engine.Actor",
        1 => "Engine.Pawn",
        2 => "Engine.PlayerPawn",
        3 => "Engine.Mover",
        4 => "Engine.ZoneInfo",
        5 => "Engine.LevelInfo",
        6 => "Engine.PlayerReplicationInfo",
        7 => "Engine.GameReplicationInfo",
        8 => "Engine.Inventory",
        9 => "Engine.Weapon",
        10 => "Engine.Ammo",
        11 => "Engine.Pickup",
        _ => return None,
    })
}

impl crate::actor_rep::RepBackend for CoreRepBackend<'_> {
    fn static_class_flags(&self, kind: u32) -> crate::actor_rep::RepResult<u32> {
        let Some(name) = rep_class_name(kind) else {
            return self.fail(RuntimeError::new("Unknown replication class kind"));
        };
        match self.access.native_class(name) {
            Ok(class) => Ok(unsafe { (*class).class_flags }),
            Err(error) => self.fail(error),
        }
    }

    fn find_rep_index(&self, kind: u32, name: &str, _site: u32) -> crate::actor_rep::RepResult<i32> {
        let Some(class_name) = rep_class_name(kind) else {
            return self.fail(RuntimeError::new("Unknown replication class kind"));
        };
        let class = match self.access.native_class(class_name) {
            Ok(class) => class,
            Err(error) => return self.fail(error),
        };
        let property_class = match self.access.native_class("Core.Property") {
            Ok(class) => class,
            Err(error) => return self.fail(error),
        };
        let name = name.split('\0').next().unwrap_or(name);
        let id = match self.access.intern_str(name, hp2_core::name_owner::FindName::Find) {
            Ok(id) => id,
            Err(error) => return self.fail(error),
        };
        let field = unsafe { self.access.find_field(class.cast::<UStruct>(), id, property_class.cast()) };
        if field.is_null() {
            return self.fail(RuntimeError::new("Replicated property was not found"));
        }
        Ok(unsafe { (*field.cast::<UProperty>()).rep_index } as i32)
    }

    fn can_serialize_object(&self, map: *mut c_void, obj: *mut c_void) -> crate::actor_rep::RepResult<bool> {
        match unsafe { can_serialize_object_core(self.access, map.cast(), obj) } {
            Ok(value) => Ok(value),
            Err(error) => self.fail(error),
        }
    }

    fn fstring_neq(&self, a: *const c_void, b: *const c_void) -> crate::actor_rep::RepResult<bool> {
        let a = unsafe { hp2_core::native_text::fstring_text(a.cast()) };
        let b = unsafe { hp2_core::native_text::fstring_text(b.cast()) };
        Ok(unsafe { hp2_core::ansi::stricmp(a.cast(), b.cast()) } != 0)
    }

    fn base_rep_list(
        &self,
        kind: u32,
        actor: *const c_void,
        recent: *const c_void,
        map: *mut c_void,
        cursor: *mut i32,
        num_reps: i32,
    ) -> crate::actor_rep::RepResult<*mut i32> {
        let mut rep = unsafe { crate::actor_rep::Rep::new_with_backend(self, map, cursor) };
        let result = unsafe {
            match kind {
                0 => crate::actor_rep::actor_get_optimized_rep_list(
                    &mut rep, actor.cast::<AActor>(), recent.cast::<AActor>()),
                1 => crate::actor_rep::pawn_get_optimized_rep_list(
                    &mut rep, actor.cast(), recent.cast(), num_reps),
                4 => crate::actor_rep::zone_info_get_optimized_rep_list(
                    &mut rep, actor.cast(), recent.cast()),
                8 => crate::actor_rep::inventory_get_optimized_rep_list(
                    &mut rep, actor.cast(), recent.cast(), num_reps),
                11 => crate::actor_rep::pickup_get_optimized_rep_list(
                    &mut rep, actor.cast(), recent.cast(), num_reps),
                _ => return self.fail(RuntimeError::new("Invalid replication superclass kind")),
            }
        };
        match result {
            Ok(()) => Ok(rep.finish()),
            Err(status) => Err(status),
        }
    }
}

unsafe fn core_optimized_rep_list(
    access: CoreAccess<'_>,
    actor: *mut c_void,
    compare: *mut u8,
    retirement: *mut c_void,
    reps: *mut i32,
    map: *mut hp2_abi::engine::generated::UPackageMap,
    num_reps: i32,
) -> RuntimeResult<(*mut i32, i32)> {
    let backend = CoreRepBackend { access, error: std::cell::RefCell::new(None) };
    let mut rep = unsafe { crate::actor_rep::Rep::new_with_backend(&backend, map.cast(), reps) };
    let mut dispatch = |name: &str| -> RuntimeResult<bool> {
        Ok(unsafe { hp2_core::object_model::is_a(actor, access.native_class(name)?.cast()) })
    };
    let result = unsafe {
        if dispatch("Engine.PlayerPawn")? {
            crate::actor_rep::player_pawn_get_optimized_rep_list(
                &mut rep, actor.cast(), compare.cast(), num_reps)
        } else if dispatch("Engine.Mover")? {
            crate::actor_rep::mover_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast())
        } else if dispatch("Engine.LevelInfo")? {
            crate::actor_rep::level_info_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast())
        } else if dispatch("Engine.PlayerReplicationInfo")? {
            crate::actor_rep::player_replication_info_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast())
        } else if dispatch("Engine.GameReplicationInfo")? {
            crate::actor_rep::game_replication_info_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast())
        } else if dispatch("Engine.Weapon")? {
            crate::actor_rep::weapon_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast(), num_reps)
        } else if dispatch("Engine.Ammo")? {
            crate::actor_rep::ammo_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast(), num_reps)
        } else if dispatch("Engine.Pickup")? {
            crate::actor_rep::pickup_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast(), num_reps)
        } else if dispatch("Engine.Inventory")? {
            crate::actor_rep::inventory_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast(), num_reps)
        } else if dispatch("Engine.ZoneInfo")? {
            crate::actor_rep::zone_info_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast())
        } else if dispatch("Engine.Pawn")? {
            crate::actor_rep::pawn_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast(), num_reps)
        } else {
            crate::actor_rep::actor_get_optimized_rep_list(&mut rep, actor.cast(), compare.cast())
        }
    };
    match result {
        Ok(()) => {
            let last = rep.finish();
            let count = ((last as isize - reps as isize) / std::mem::size_of::<i32>() as isize) as i32;
            Ok((last, count))
        }
        Err(_) => Err(backend.take_error()),
    }
}

impl crate::net_chan::ActorReplicateOps for CoreActorOps<'_> {
    unsafe fn with_out_bunch<T>(
        &mut self,
        channel: *mut UChannel,
        close: i32,
        body: impl FnOnce(&mut Self, *mut FOutBunch) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let mut failure = None;
        let result = unsafe {
            crate::net_bunch::with_out_core(channel, close, |bunch| {
                match body(self, bunch) {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        failure = Some(error);
                        Err(crate::net_chan::Thrown)
                    }
                }
            })
        };
        match result {
            Ok(value) => Ok(value),
            Err(_) => Err(failure.unwrap_or_else(|| {
                RuntimeError::new("Core actor-replication bunch allocation failed")
            })),
        }
    }

    unsafe fn with_mem_scope<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let memory = self.access.scratch_memory();
        let mark = unsafe { memory.mark() };
        let result = body(self);
        unsafe { memory.pop(&mark); }
        result
    }

    unsafe fn out_bunch_is_error(&mut self, bunch: *mut FOutBunch) -> RuntimeResult<bool> {
        Ok(unsafe { (*bunch).base.ar_is_error != 0 })
    }

    unsafe fn array_num(&mut self, array: *const TArrayRaw) -> RuntimeResult<i32> {
        Ok(unsafe { (*array).num })
    }

    unsafe fn retirement_get(&mut self, channel: *mut UActorChannel, index: i32) -> RuntimeResult<*mut c_void> {
        <Self as crate::net_chan::ActorReceiveOps>::retirement_get(self, channel, index)
    }

    unsafe fn retirement_out_packet(&mut self, retirement: *mut c_void) -> RuntimeResult<i32> {
        if retirement.is_null() { return Err(RuntimeError::new("Null retirement entry")); }
        Ok(unsafe { (*retirement.cast::<PropertyRetirement>()).out_packet_id })
    }

    unsafe fn retirement_reliable(&mut self, retirement: *mut c_void) -> RuntimeResult<i32> {
        if retirement.is_null() { return Err(RuntimeError::new("Null retirement entry")); }
        Ok(unsafe { (*retirement.cast::<PropertyRetirement>()).reliable as i32 })
    }

    unsafe fn array_add_unique_i32(&mut self, array: *mut TArrayRaw, value: i32) -> RuntimeResult<()> {
        let count = unsafe { (*array).num };
        if count < 0 || unsafe { (*array).max } < count || (count > 0 && unsafe { (*array).data }.is_null()) {
            return Err(RuntimeError::new("Invalid actor-channel array"));
        }
        for index in 0..count {
            if unsafe { (*array).data.cast::<i32>().add(index as usize).read_unaligned() } == value {
                return Ok(());
            }
        }
        let added = unsafe {
            hp2_core::array_storage::add(
                array_slots(array), 1, 4, ptr::null(), hp2_core::array_storage::system_allocator(),
            )
        }.map_err(|_| RuntimeError::new("Actor-channel unique append failed"))?;
        unsafe { (*array).data.cast::<i32>().add(added as usize).write_unaligned(value); }
        Ok(())
    }

    unsafe fn actor_is_player_pawn(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        <Self as crate::net_chan::ActorReceiveOps>::actor_is_player_pawn(self, actor)
    }
    unsafe fn actor_player(&mut self, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        <Self as crate::net_chan::ActorReceiveOps>::actor_player(self, actor)
    }
    unsafe fn player_is_viewport(&mut self, player: *mut c_void) -> RuntimeResult<bool> {
        <Self as crate::net_chan::ActorReceiveOps>::player_is_viewport(self, player)
    }

    unsafe fn driver_recording_demo(&mut self, driver: *mut UNetDriver) -> RuntimeResult<bool> {
        let class = self.access.native_class("Engine.DemoRecDriver")?;
        Ok(!driver.is_null() && unsafe { hp2_core::object_model::is_a(driver.cast(), class.cast()) })
    }

    unsafe fn actor_net_mode_client(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor net-mode query")); }
        let level = unsafe { (*actor.cast::<AActor>()).level.cast::<hp2_abi::engine::generated::ALevelInfo>() };
        if level.is_null() { return Err(RuntimeError::new("Actor has no LevelInfo")); }
        Ok(unsafe { (*level).net_mode } == 3)
    }

    unsafe fn pm_serialize_object(
        &mut self,
        map: *mut UPackageMap,
        bunch: *mut FOutBunch,
        object: *mut *mut c_void,
    ) -> RuntimeResult<bool> {
        let writer = CoreOutBunchWriter {
            access: self.access,
            package_map: map.cast(),
            bunch,
        };
        Ok(unsafe { hp2_core::object_archive::NetworkWriter::write_object(&writer, object.cast(), ptr::null_mut())? } != 0)
    }

    unsafe fn out_bunch_serialize_vector(&mut self, bunch: *mut FOutBunch, vector: *mut FVector) -> RuntimeResult<()> {
        if bunch.is_null() || vector.is_null() {
            return Err(RuntimeError::new("Null actor location serialization"));
        }
        unsafe { hp2_core::bits::writer_serialize(writer_of(bunch), vector.cast(), std::mem::size_of::<FVector>() as i32); }
        Ok(())
    }

    unsafe fn actor_roles(&mut self, actor: *mut c_void) -> RuntimeResult<(u8, u8)> {
        <Self as crate::net_chan::ActorReceiveOps>::actor_roles(self, actor)
    }

    unsafe fn mem_new_int(&mut self, count: i32) -> RuntimeResult<*mut i32> {
        if count < 0 { return Err(RuntimeError::new("Negative network scratch count")); }
        let bytes = count.checked_mul(4).ok_or_else(|| RuntimeError::new("Network scratch overflow"))?;
        let pointer = unsafe { self.access.scratch_memory().push_bytes(bytes, 4)? };
        unsafe { ptr::write_bytes(pointer, 0, bytes as usize); }
        Ok(pointer.cast())
    }

    unsafe fn actor_optimized_rep_list(
        &mut self,
        actor: *mut c_void,
        compare: *mut u8,
        retirement: *mut c_void,
        reps: *mut i32,
        map: *mut UPackageMap,
        num_reps: i32,
    ) -> RuntimeResult<(*mut i32, i32)> {
        let _ = retirement;
        unsafe { core_optimized_rep_list(self.access, actor, compare, retirement, reps, map, num_reps) }
    }

    unsafe fn actor_should_script_replicate(&mut self, actor: *mut c_void) -> RuntimeResult<bool> {
        let class = self.access.native_class("Engine.Inventory")?;
        if !actor.is_null() && unsafe { hp2_core::object_model::is_a(actor, class.cast()) } {
            Ok(unsafe { crate::actor_rep::inventory_should_do_script_replication(actor.cast::<AInventory>()) })
        } else {
            Ok(true)
        }
    }

    unsafe fn cache_rep_property_at(&mut self, cache: *mut FClassNetCache, index: i32) -> RuntimeResult<*mut FFieldNetCache> {
        if cache.is_null() || index < 0 || index >= unsafe { (*cache).rep_properties.num } {
            return Err(RuntimeError::new("Replication property index is out of range"));
        }
        let data = unsafe { (*cache).rep_properties.data.cast::<*mut c_void>() };
        if data.is_null() { return Err(RuntimeError::new("Replication property array is missing")); }
        Ok(unsafe { data.add(index as usize).read_unaligned().cast() })
    }

    unsafe fn property_is_object(&mut self, property: *mut UProperty) -> RuntimeResult<bool> {
        unsafe { self.is_class(property.cast(), "Core.ObjectProperty") }
    }

    unsafe fn can_serialize_object(&mut self, map: *mut UPackageMap, object_value: *mut c_void) -> RuntimeResult<bool> {
        let object = unsafe { object_value.cast::<*mut c_void>().read_unaligned() };
        unsafe { can_serialize_object_core(self.access, map, object) }
    }

    unsafe fn property_identical(&mut self, property: *mut UProperty, a: *mut u8, b: *mut u8) -> RuntimeResult<bool> {
        let status = unsafe {
            self.access.property_value(
                hp2_core::property_value::Hp2ValueOperation::ValueIdentical,
                property,
                a.cast(),
                b.cast(),
                0,
                None,
            )?
        };
        Ok(status != 0)
    }

    unsafe fn property_replicated_metadata(&mut self, property: *mut UProperty) -> RuntimeResult<(*mut c_void, i32)> {
        if property.is_null() { return Err(RuntimeError::new("Null replicated property")); }
        let owner = unsafe { (*property).rep_owner };
        if owner.is_null() { return Err(RuntimeError::new("Replicated property has no RepOwner")); }
        Ok((owner, unsafe { (*owner.cast::<UProperty>()).rep_offset } as i32))
    }

    unsafe fn property_owner_class(&mut self, property: *mut UProperty) -> RuntimeResult<*mut UClass> {
        <Self as crate::net_chan::ActorReceiveOps>::property_owner_class(self, property)
    }

    unsafe fn frame_step_condition(&mut self, actor: *mut c_void, owner_class: *mut UClass, offset: i32) -> RuntimeResult<u32> {
        <Self as crate::net_chan::ActorReceiveOps>::frame_step_condition(self, actor, owner_class, offset)
    }

    unsafe fn class_rep_record(&mut self, class: *mut UClass, index: i32) -> RuntimeResult<*mut c_void> {
        if class.is_null() { return Err(RuntimeError::new("Null replicated actor class")); }
        let records = unsafe { (*class).class_reps };
        if index < 0 || index >= records.num || records.data.is_null() {
            return Err(RuntimeError::new("Class replication record index is out of range"));
        }
        Ok(unsafe { records.data.cast::<FRepRecord>().add(index as usize).cast() })
    }

    unsafe fn rep_record_metadata(&mut self, record: *mut c_void) -> RuntimeResult<(*mut UProperty, i32)> {
        if record.is_null() { return Err(RuntimeError::new("Null class replication record")); }
        let record = unsafe { &*record.cast::<FRepRecord>() };
        Ok((record.property.cast(), record.index))
    }

    unsafe fn property_name_kind(&mut self, property: *mut UProperty) -> RuntimeResult<i32> {
        if property.is_null() { return Err(RuntimeError::new("Null replicated property name")); }
        let name = i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name });
        if name == self.access.intern_str("Role", hp2_core::name_owner::FindName::Find)? {
            Ok(1)
        } else if name == self.access.intern_str("RemoteRole", hp2_core::name_owner::FindName::Find)? {
            Ok(2)
        } else {
            Ok(0)
        }
    }

    unsafe fn cache_from_field(&mut self, cache: *mut FClassNetCache, field: *mut c_void) -> RuntimeResult<*mut FFieldNetCache> {
        let mut current = cache;
        while !current.is_null() {
            let fields = unsafe { (*current).fields };
            if fields.num < 0 || (fields.num > 0 && fields.data.is_null()) {
                return Err(RuntimeError::new("Invalid class-cache fields"));
            }
            for index in 0..fields.num {
                let entry = unsafe { fields.data.cast::<FFieldNetCache>().add(index as usize) };
                if unsafe { (*entry).field } == field {
                    return Ok(entry);
                }
            }
            current = unsafe { (*current).super_.cast() };
        }
        Err(RuntimeError::new("Field has no replication cache entry"))
    }

    unsafe fn cache_max_index(&mut self, cache: *mut FClassNetCache) -> RuntimeResult<i32> {
        <Self as crate::net_chan::ActorReceiveOps>::cache_max_index(self, cache)
    }

    unsafe fn bunch_write_int(&mut self, bunch: *mut FOutBunch, value: i32, max_index: i32) -> RuntimeResult<()> {
        if max_index < 0 { return Err(RuntimeError::new("Negative replication field bound")); }
        unsafe {
            hp2_core::bits::writer_write_int_with_checker(
                writer_of(bunch),
                value as u32,
                max_index as u32,
                hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks,
                bit_error,
            )
        }
    }

    unsafe fn bunch_write_byte(&mut self, bunch: *mut FOutBunch, value: u8) -> RuntimeResult<()> {
        unsafe { hp2_core::bits::writer_serialize(writer_of(bunch), (&raw const value).cast(), 1); }
        Ok(())
    }

    unsafe fn property_net_serialize(
        &mut self,
        property: *mut UProperty,
        bunch: *mut FOutBunch,
        package_map: *mut UPackageMap,
        data: *mut u8,
    ) -> RuntimeResult<bool> {
        let writer = CoreOutBunchWriter {
            access: self.access,
            package_map: package_map.cast(),
            bunch,
        };
        let mapping = CoreOutBunchMapping {
            access: self.access,
            package_map: package_map.cast(),
        };
        let flags = hp2_core::object_archive::ArchiveFlags::new(
            unsafe { (*writer_of(bunch)).ar_ver as u16 },
            unsafe { (*writer_of(bunch)).ar_licensee_ver as u16 },
            unsafe { (*writer_of(bunch)).ar_is_persistent != 0 },
        );
        let archive = hp2_core::object_archive::ObjectArchive::network_saving(&writer, flags, &mapping);
        let status = unsafe {
            self.access.property_net_serialize(
                property,
                package_map.cast(),
                &archive,
                data.cast(),
                ptr::null_mut(),
                0,
            )?
        };
        Ok(status != 0)
    }

    unsafe fn property_copy_single(&mut self, property: *mut UProperty, destination: *mut u8, source: *mut u8) -> RuntimeResult<()> {
        <Self as crate::net_chan::ActorReceiveOps>::property_copy_single(self, property, destination, source)
    }

    unsafe fn actor_level_num_reps_add(&mut self, actor: *mut c_void) -> RuntimeResult<()> {
        if actor.is_null() { return Err(RuntimeError::new("Null actor level NumReps update")); }
        let level = unsafe { (*actor.cast::<AActor>()).x_level.cast::<ULevel>() };
        if level.is_null() { return Err(RuntimeError::new("Actor has no XLevel")); }
        unsafe { (*level).num_reps = (*level).num_reps.wrapping_add(1); }
        Ok(())
    }

    unsafe fn bunch_mark_pop(&mut self, bunch: *mut FOutBunch, overflowed: i32, num: i32) -> RuntimeResult<()> {
        unsafe {
            hp2_core::bits::writer_mark_pop_with_checker(
                writer_of(bunch), overflowed, num, bit_error,
            )
        }
    }

    unsafe fn mark_num_bits(&mut self, bunch: *mut FOutBunch) -> RuntimeResult<i32> {
        Ok(unsafe { (*writer_of(bunch)).num })
    }

    unsafe fn send_bunch(&mut self, channel: *mut UChannel, bunch: *mut FOutBunch, merge: i32) -> RuntimeResult<i32> {
        unsafe { crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, merge) }
    }

    unsafe fn array_remove_item_i32(&mut self, array: *mut TArrayRaw, index: i32) -> RuntimeResult<()> {
        unsafe {
            hp2_core::array_storage::remove(
                array_slots(array), index, 1, 4, ptr::null(), hp2_core::array_storage::system_allocator(),
            )
        }.map_err(|_| RuntimeError::new("Actor-channel dirty removal failed"))
    }

    unsafe fn retirement_set_out(&mut self, channel: *mut UActorChannel, index: i32, packet: i32) -> RuntimeResult<()> {
        let retirement = unsafe { (*channel).retirement };
        if index < 0 || index >= retirement.num || retirement.data.is_null() {
            return Err(RuntimeError::new("Retirement index is outside actor channel storage"));
        }
        unsafe {
            (*retirement.data.cast::<PropertyRetirement>().add(index as usize)).out_packet_id = packet;
        }
        Ok(())
    }

    unsafe fn retirement_set_reliable(&mut self, retirement: *mut c_void, reliable: i32) -> RuntimeResult<()> {
        if retirement.is_null() { return Err(RuntimeError::new("Null retirement entry")); }
        unsafe { (*retirement.cast::<PropertyRetirement>()).reliable = reliable as u8; }
        Ok(())
    }

    unsafe fn conn_add_sent_temporary(&mut self, connection: *mut UNetConnection, actor: *mut c_void) -> RuntimeResult<()> {
        let array = unsafe { ptr::addr_of_mut!((*connection).sent_temporaries) };
        let added = unsafe {
            hp2_core::array_storage::add(
                array_slots(array), 1, std::mem::size_of::<*mut c_void>() as i32,
                ptr::null(), hp2_core::array_storage::system_allocator(),
            )
        }.map_err(|_| RuntimeError::new("SentTemporaries append failed"))?;
        unsafe { (*array).data.cast::<*mut c_void>().add(added as usize).write_unaligned(actor); }
        Ok(())
    }

    unsafe fn driver_time_into(&mut self, driver: *mut UNetDriver, destination: *mut u8) -> RuntimeResult<()> {
        if driver.is_null() || destination.is_null() {
            return Err(RuntimeError::new("Invalid driver time update"));
        }
        unsafe {
            ptr::copy_nonoverlapping(ptr::addr_of!((*driver).time).cast::<u8>(), destination, 8);
        }
        Ok(())
    }
}

/// Assigns an actor through the canonical `UActorChannel` policy.
///
/// # Safety
/// `ch`, `actor` and their connection/package map must remain live.
pub(crate) unsafe fn set_actor(
    access: CoreAccess<'_>,
    ch: *mut UActorChannel,
    actor: *mut AActor,
) -> RuntimeResult<()> {
    let mut ops = CoreActorOps { access };
    unsafe { crate::net_chan::set_actor_with(ch, actor.cast(), &mut ops) }
}

/// Reports the canonical channel's packet readiness.
///
/// # Safety
/// `ch` and its connection output state must remain live.
pub(crate) unsafe fn is_net_ready(
    access: CoreAccess<'_>,
    ch: *mut UActorChannel,
    saturate: i32,
) -> RuntimeResult<i32> {
    unsafe { crate::net_chan::channel_is_net_ready_core(access, ch.cast(), saturate) }
}

/// Runs `UActorChannel::ReplicateActor` with Core-owned engine records.
///
/// # Safety
/// `ch`, its actor, connection and package map must remain live throughout.
pub(crate) unsafe fn replicate_actor(
    access: CoreAccess<'_>,
    ch: *mut UActorChannel,
) -> RuntimeResult<()> {
    let mut ops = CoreActorOps { access };
    unsafe { crate::net_chan::actor_replicate_actor_with(ch, &mut ops) }
}

/// Encodes and sends one canonical Actor RPC through its actor channel.
///
/// # Safety
/// The actor, function, parameter storage, optional VM stack and connection graph remain live.
pub(crate) unsafe fn send_remote_function(
    access: CoreAccess<'_>,
    actor: *mut AActor,
    connection: *mut UNetConnection,
    function: *mut UFunction,
    parameters: *mut c_void,
    stack: *mut hp2_abi::engine::generated::FFrame,
    is_server: bool,
) -> RuntimeResult<()> {
    if actor.is_null() || connection.is_null() || function.is_null() {
        return Err(RuntimeError::new("Invalid canonical Actor RPC send arguments"));
    }
    let level = unsafe { (*actor).x_level.cast::<ULevel>() };
    if level.is_null() { return Err(RuntimeError::new("Actor has no XLevel during RPC send")); }
    unsafe { (*level).num_rpc = (*level).num_rpc.wrapping_add(1); }
    let package_map = unsafe { (*connection).package_map.cast::<UPackageMap>() };
    let class = unsafe { (*actor.cast::<UObject>()).class.cast::<UClass>() };
    let mut ops = CoreActorOps { access };
    let cache = unsafe { <CoreActorOps<'_> as crate::net_chan::ActorSetActorOps>::class_net_cache(&mut ops, package_map, class)? };
    if cache.is_null() { return Ok(()); }
    let mut current = cache;
    let mut field = ptr::null_mut();
    while !current.is_null() {
        let fields = unsafe { (*current).fields };
        if fields.num < 0 || (fields.num > 0 && fields.data.is_null()) {
            return Err(RuntimeError::new("Invalid RPC class-cache fields"));
        }
        for index in 0..fields.num {
            let entry = unsafe { fields.data.cast::<FFieldNetCache>().add(index as usize) };
            if unsafe { (*entry).field } == function.cast() { field = entry; break; }
        }
        if !field.is_null() { break; }
        current = unsafe { (*current).super_.cast() };
    }
    if field.is_null() { return Ok(()); }
    let mut channel = unsafe { crate::network_owner::actor_channel(connection, actor)? }.cast::<UActorChannel>();
    if channel.is_null() {
        if !is_server { return Ok(()); }
        channel = unsafe {
            crate::connection_owner::create_channel(
                access, connection, crate::level_net::CHTYPE_ACTOR, 1, -1,
            )?.cast()
        };
        unsafe { set_actor(access, channel, actor)?; }
    }
    if channel.is_null() { return Ok(()); }
    let base = channel.cast::<UChannel>();
    if unsafe { (*base).open_packet_id } == -1 {
        if !is_server { return Ok(()); }
        unsafe { replicate_actor(access, channel)?; }
    }
    let field_index = unsafe { (*field).field_net_index };
    let max_index = unsafe { <CoreActorOps<'_> as crate::net_chan::ActorReceiveOps>::cache_max_index(&mut ops, cache)? };
    let mut body_error = None;
    let result = unsafe {
        crate::net_bunch::with_out_core(base, 0, |bunch| {
            let body = (|| -> RuntimeResult<()> {
                <CoreActorOps<'_> as crate::net_chan::ActorReplicateOps>::bunch_write_int(
                    &mut ops, bunch, field_index, max_index,
                )?;
                if !stack.is_null() {
                    let size = (*function).parms_size as usize;
                    if size != 0 {
                        if parameters.is_null() { return Err(RuntimeError::new("RPC stack has null parameter storage")); }
                        ptr::write_bytes(parameters.cast::<u8>(), 0, size);
                    }
                    let property_class = access.native_class("Core.Property")?;
                    let mut property = next_property_field((*function.cast::<UStruct>()).children.cast(), property_class);
                    while !property.is_null()
                        && (*property).property_flags & (
                            hp2_core::class_metadata::METADATA_CPF_PARM
                            | hp2_core::class_metadata::METADATA_CPF_RETURN
                        ) == hp2_core::class_metadata::METADATA_CPF_PARM
                    {
                        let destination = parameters.cast::<u8>().add((*property).offset as usize);
                        access.vm_step_argument(stack, destination.cast())?;
                        property = next_property_field((*property.cast::<UField>()).next.cast(), property_class);
                    }
                    if (*stack).code.is_null()
                        || (*stack).code.read() != hp2_abi::engine::generated::EX_EndFunctionParms as u8
                    {
                        return Err(RuntimeError::new("Actor RPC parameter frame has no end-of-parameters token"));
                    }
                }
                let bool_class = access.native_class("Core.BoolProperty")?;
                let property_class = access.native_class("Core.Property")?;
                let mut property = next_property_field((*function.cast::<UStruct>()).children.cast(), property_class);
                while !property.is_null()
                    && (*property).property_flags & (
                        hp2_core::class_metadata::METADATA_CPF_PARM
                        | hp2_core::class_metadata::METADATA_CPF_RETURN
                    ) == hp2_core::class_metadata::METADATA_CPF_PARM
                {
                        if parameters.is_null() { return Err(RuntimeError::new("Actor RPC has null parameter storage")); }
                        if access.package_map_object_to_index(package_map, property.cast())? != crate::prim::INDEX_NONE {
                            let data = parameters.cast::<u8>().add((*property).offset as usize);
                            let is_bool = hp2_core::object_model::is_a(property.cast(), bool_class.cast());
                            if !is_bool {
                                let identical = access.property_value(
                                    hp2_core::property_value::Hp2ValueOperation::ValueMatches,
                                    property, data.cast(), ptr::null_mut(), 0, None,
                                )?;
                                let send = identical == 0;
                                hp2_core::bits::writer_write_bit(bunch.cast(), u8::from(send));
                                if send {
                                    let _ = <CoreActorOps<'_> as crate::net_chan::ActorReplicateOps>::property_net_serialize(
                                        &mut ops, property, bunch, package_map, data,
                                    )?;
                                }
                            } else {
                                let _ = <CoreActorOps<'_> as crate::net_chan::ActorReplicateOps>::property_net_serialize(
                                    &mut ops, property, bunch, package_map, data,
                                )?;
                            }
                        }
                    property = next_property_field((*property.cast::<UField>()).next.cast(), property_class);
                }
                if (*function).function_flags & 0x80 != 0 { (*bunch).b_reliable = 1; }
                if (*bunch).base.ar_is_error == 0 {
                    crate::net_chan::channel_send_bunch_core(access, base, bunch, 1)?;
                }
                Ok(())
            })();
            body.map_err(|error| {
                body_error = Some(error);
                crate::net_chan::Thrown
            })
        })
    };
    if result.is_err() {
        return Err(body_error.unwrap_or_else(|| RuntimeError::new("Actor RPC bunch processing failed")));
    }
    Ok(())
}

unsafe fn visit_actor_channel(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &hp2_core::object_archive::ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { (OBJECT.visit_references)(access, object, archive)?; }
    let channel = object.cast::<UActorChannel>();
    unsafe {
        archive.serialize_object_slot(ptr::addr_of_mut!((*channel.cast::<UChannel>()).connection).cast())?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*channel).level).cast())?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*channel).actor).cast())?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*channel).actor_class).cast())?;
    }
    Ok(())
}
unsafe fn destroy_actor_channel(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical actor channel"));
    }
    let channel = object.cast::<UActorChannel>();
    let mut first_error = unsafe { remove_actor_channel_entry(channel) }.err();
    unsafe {
        if (*channel).recent.num != 0 {
            let class = (*channel).actor_class;
            if class.is_null() {
                if first_error.is_none() {
                    first_error = Some(RuntimeError::new("Actor-channel recent data has no class"));
                }
            } else {
                let mut property = (*class.cast::<UStruct>()).constructor_link;
                while !property.is_null() {
                    let metadata = property.cast::<UProperty>();
                    if (*metadata).property_flags & 8 != 0 {
                        let value = (*channel).recent.data.cast::<u8>()
                            .wrapping_offset((*metadata).offset as isize)
                            .cast();
                        let result = access.property_value(
                            hp2_core::property_value::Hp2ValueOperation::ValueDestroy,
                            property.cast::<UProperty>(),
                            value,
                            ptr::null_mut(),
                            0,
                            None,
                        ).and_then(|status| {
                            if status == 0 {
                                Err(RuntimeError::new("Actor-channel recent property destruction failed"))
                            } else {
                                Ok(())
                            }
                        });
                        if let Err(error) = result {
                            if first_error.is_none() { first_error = Some(error); }
                        }
                    }
                    property = (*metadata).constructor_link_next;
                }
            }
        }
        for array in [
            ptr::addr_of_mut!((*channel).recent),
            ptr::addr_of_mut!((*channel).rep_eval),
            ptr::addr_of_mut!((*channel).dirty),
            ptr::addr_of_mut!((*channel).retirement),
        ] {
            if crate::net_bunch::release_core_buffer(array).is_err() && first_error.is_none() {
                first_error = Some(RuntimeError::new("Actor-channel array release failed"));
            }
        }
    }
    let base = unsafe { crate::channel_owner::destroy_channel_body(access, channel.cast()) };
    match first_error {
        Some(error) => { let _ = base; Err(error) }
        None => base,
    }
}
unsafe fn remove_actor_channel_entry(channel: *mut UActorChannel) -> RuntimeResult<()> {
    let base = channel.cast::<UChannel>();
    let connection = unsafe { (*base).connection.cast::<UNetConnection>() };
    let actor = unsafe { (*channel).actor };
    if connection.is_null() || actor.is_null() {
        return Ok(());
    }
    let map = unsafe { ptr::addr_of_mut!((*connection).actor_channels) };
    let array = unsafe { ptr::addr_of_mut!((*map).pairs) };
    let pairs = unsafe { (*array).data.cast::<FActorChannelMapPair>() };
    let mut index = 0;
    while index < unsafe { (*array).num } {
        let pair = unsafe { pairs.add(index as usize) };
        if unsafe { (*pair).key == actor && (*pair).value == channel.cast() } {
            let slots = Hp2ArraySlots {
                data: unsafe { ptr::addr_of_mut!((*array).data).cast() },
                num: unsafe { ptr::addr_of_mut!((*array).num) },
                max: unsafe { ptr::addr_of_mut!((*array).max) },
            };
            let hash_count = unsafe { (*map).hash_count };
            let hash = unsafe { (*map).hash.cast::<i32>() };
            if hash_count <= 0 || !(hash_count as u32).is_power_of_two() || hash.is_null() {
                return Err(RuntimeError::new("Invalid actor-channel map hash storage"));
            }
            unsafe {
                hp2_core::array_storage::remove(
                    slots,
                    index,
                    1,
                    std::mem::size_of::<FActorChannelMapPair>() as i32,
                    ptr::null(),
                    hp2_core::array_storage::system_allocator(),
                )
            }.map_err(|_| RuntimeError::new("Actor-channel map removal failed"))?;
            let hash_count = unsafe { (*map).hash_count };
            let hash = unsafe { (*map).hash.cast::<i32>() };
            if hash_count <= 0 || !(hash_count as u32).is_power_of_two() || hash.is_null() {
                return Err(RuntimeError::new("Invalid actor-channel map hash storage"));
            }
            for bucket in 0..hash_count as usize {
                unsafe { hash.add(bucket).write_unaligned(-1); }
            }
            let pairs = unsafe { (*array).data.cast::<FActorChannelMapPair>() };
            for pair_index in 0..unsafe { (*array).num } {
                let pair = unsafe { pairs.add(pair_index as usize) };
                let key = unsafe { (*pair).key };
                if key.is_null() {
                    return Err(RuntimeError::new("Null actor-channel map key"));
                }
                let bucket = (unsafe { (*key.cast::<UObject>()).index } & (hash_count - 1)) as usize;
                unsafe {
                    (*pair).hash_next = hash.add(bucket).read_unaligned();
                    hash.add(bucket).write_unaligned(pair_index);
                }
            }
            return Ok(());
        }
        index += 1;
    }
    Err(RuntimeError::new("ActorChannels.Remove(Actor) failed"))
}
