//! Rust entry points for `UnChan.cpp`.
//!
//! Rust owns bunch allocation, buffers, copies and scoped FString/mark values.
//! C++ retains SDK archive identity and guarded typed engine calls.
//! Every `chan_*` callback which can execute engine code returns the
//! `Hp2FfiCall` status: zero succeeds and nonzero means that a C++ exception
//! was captured. Cleanup still runs and preserves the first captured error.
use hp2_abi::FVector;
use hp2_abi::engine::generated::{
    AActor, FClassNetCache, FFieldNetCache, FInBunch, FMemStack, FName, FOutBunch, FRotator, TArrayRaw,
    UActorChannel, UBoolProperty, UChannel, UClass, UDownload, UFileChannel, UFunction,
    UNetConnection, UNetDriver, UPackageMap, UProperty, UStruct,
};
use std::{ffi::{c_char, c_void}, ptr};
use crate::channel_owner::{self, CloseOps};
use crate::net_bunch::{self, Hp2NetBunchServices};

#[derive(Clone, Copy, Debug)]
pub struct Thrown;

/// Base-channel tick and packet-readiness primitives shared by native and Core
/// channel owners. The protocol sequencing remains in the generic policies below.
pub trait ChannelTickOps {
    type Error;

    unsafe fn check_failed(&self, expression: &std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn driver_time_since(
        &self,
        driver: *mut UNetDriver,
        time: *const u8,
    ) -> Result<f32, Self::Error>;
    unsafe fn send_raw_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error>;
    unsafe fn connection_is_net_ready(
        &self,
        connection: *mut UNetConnection,
        saturate: i32,
    ) -> Result<i32, Self::Error>;
    unsafe fn connection_out_num_bits(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<i32, Self::Error>;
}

/// File-channel send operations around the shared Tick/packet/chunk policy.
pub trait FileTickOps: ChannelTickOps {
    unsafe fn package_file_size(
        &self,
        connection: *mut UNetConnection,
        package_index: i32,
    ) -> Result<i32, Self::Error>;
    unsafe fn archive_serialize(
        &self,
        archive: *mut c_void,
        data: *mut u8,
        count: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn archive_is_error(&self, archive: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn bunch_serialize_bytes(
        &self,
        bunch: *mut FOutBunch,
        data: *mut u8,
        count: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Self::Error>;
    unsafe fn with_out_bunch<F>(
        &self,
        channel: *mut UChannel,
        close: i32,
        body: F,
    ) -> Result<(), Self::Error>
    where
        F: FnOnce(*mut FOutBunch) -> Result<(), Self::Error>;
    unsafe fn send_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error>;
    unsafe fn flush_net(&self, connection: *mut UNetConnection) -> Result<(), Self::Error>;
    unsafe fn delete_archive(&self, archive: *mut c_void) -> Result<(), Self::Error>;
}

// The canonical Rust arena already owns GMem chunks. Keep the mark on the
// stack, including its nested typed-callback allocations, on every exit path.
struct NetworkMemScope(hp2_abi::render_span::FMemMark);
impl NetworkMemScope {
    unsafe fn new(stack: *mut FMemStack) -> Self {
        let mut mark = std::mem::MaybeUninit::uninit();
        unsafe {
            hp2_core::mem::mark_init(mark.as_mut_ptr(), stack);
            Self(mark.assume_init())
        }
    }
}
impl Drop for NetworkMemScope {
    fn drop(&mut self) {
        unsafe { hp2_core::mem::mark_pop(&self.0); }
    }
}

/// C++ operations required by the Rust `UnChan.cpp` bodies.
///
/// Pointer-returning and scalar-returning operations use out parameters so
/// their C++ thunk can report a captured exception without crossing the plain
/// C ABI.  The derived-channel callbacks are deliberately scoped to source
/// blocks whose values are opaque to generated mirrors (archives, FStrings,
/// reflection and `TMap`); their C++ definitions retain precisely the oracle
/// expressions named by the field comments.
#[repr(C)]
pub struct Hp2NetChanServices {
    /// Narrow C++ leaf adapter for chan_check_failed.
    pub chan_check_failed: unsafe extern "C" fn(*const c_char, *const c_char, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_out_num_bits.
    pub chan_conn_out_num_bits: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_out_num_bytes.
    pub chan_conn_out_num_bytes: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_last_out.
    pub chan_conn_last_out: unsafe extern "C" fn(*mut c_void, *mut *mut FOutBunch) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_last_start.
    pub chan_conn_last_start: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_last_end.
    pub chan_conn_last_end: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_last_out_bunch_get.
    pub chan_conn_last_out_bunch_get: unsafe extern "C" fn(*mut c_void, *mut *mut FOutBunch) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_last_out_bunch_set.
    pub chan_conn_last_out_bunch_set: unsafe extern "C" fn(*mut c_void, *mut FOutBunch) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_send_raw_bunch.
    pub chan_conn_send_raw_bunch:
        unsafe extern "C" fn(*mut c_void, *mut FOutBunch, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_is_net_ready.
    pub chan_conn_is_net_ready: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_tarray_remove_channel.
    pub chan_tarray_remove_channel:
        unsafe extern "C" fn(*mut TArrayRaw, *mut c_void, *mut i32) -> i32,
    pub bunches: Hp2NetBunchServices,
    /// Narrow C++ leaf adapter for chan_mark_pop_last_start.
    pub chan_mark_pop_last_start: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_mark_set_last_end.
    pub chan_mark_set_last_end: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_is_error.
    pub chan_out_bunch_is_error: unsafe extern "C" fn(*mut FOutBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_num_bits.
    pub chan_out_bunch_num_bits: unsafe extern "C" fn(*mut FOutBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_num_bytes.
    pub chan_out_bunch_num_bytes: unsafe extern "C" fn(*mut FOutBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_data.
    pub chan_out_bunch_data: unsafe extern "C" fn(*mut FOutBunch, *mut *const u8) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_serialize_bits.
    pub chan_out_bunch_serialize_bits: unsafe extern "C" fn(*mut FOutBunch, *const u8, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_pm_serialize_object.
    pub chan_pm_serialize_object:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_pm_serialize_name.
    pub chan_pm_serialize_name: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_virtual_received_bunch.
    pub chan_virtual_received_bunch: unsafe extern "C" fn(*mut c_void, *mut FInBunch) -> i32,
    /// Narrow C++ leaf adapter for chan_virtual_set_closing_flag.
    pub chan_virtual_set_closing_flag: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_channel_delete.
    pub chan_channel_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_uobject_destroy.
    pub chan_uobject_destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_object_get_flags.
    pub chan_object_get_flags: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    /// Narrow C++ leaf adapter for chan_object_clear_flags.
    pub chan_object_clear_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    /// Narrow C++ leaf adapter for chan_object_set_flags.
    pub chan_object_set_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_conditional_destroy.
    pub chan_conn_conditional_destroy: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_driver_time_since.
    pub chan_driver_time_since: unsafe extern "C" fn(*mut c_void, *const u8, *mut f32) -> i32,
    /// Narrow C++ leaf adapter for chan_driver_notify_get_level.
    pub chan_driver_notify_get_level: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_driver_time_into.
    pub chan_driver_time_into: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_driver_time_minus_spawn_priority.
    pub chan_driver_time_minus_spawn_priority: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_channel_class_at.
    pub chan_channel_class_at: unsafe extern "C" fn(i32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_in_bunch_serialize_string.
    pub chan_in_bunch_serialize_string: unsafe extern "C" fn(*mut FInBunch, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_in_bunch_is_error.
    pub chan_in_bunch_is_error: unsafe extern "C" fn(*mut FInBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_notify_received_text.
    pub chan_notify_received_text: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_serialize_string.
    pub chan_out_bunch_serialize_string: unsafe extern "C" fn(*mut FOutBunch, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_actor_channels_remove.
    pub chan_conn_actor_channels_remove: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_array_num.
    pub chan_array_num: unsafe extern "C" fn(*const TArrayRaw, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_add.
    pub chan_array_add: unsafe extern "C" fn(*mut TArrayRaw, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_add_zeroed.
    pub chan_array_add_zeroed: unsafe extern "C" fn(*mut TArrayRaw, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_empty.
    pub chan_array_empty: unsafe extern "C" fn(*mut TArrayRaw, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_remove_item_i32.
    pub chan_array_remove_item_i32: unsafe extern "C" fn(*mut TArrayRaw, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_add_unique_i32.
    pub chan_array_add_unique_i32: unsafe extern "C" fn(*mut TArrayRaw, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_i32_at.
    pub chan_array_i32_at: unsafe extern "C" fn(*const TArrayRaw, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_i32_set.
    pub chan_array_i32_set: unsafe extern "C" fn(*mut TArrayRaw, i32, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_array_ptr_at.
    pub chan_array_ptr_at: unsafe extern "C" fn(*const TArrayRaw, i32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_array_data.
    pub chan_array_data: unsafe extern "C" fn(*mut TArrayRaw, *mut *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_get_class.
    pub chan_actor_get_class: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_flags.
    pub chan_actor_flags:
        unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32, *mut i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_get_roles.
    pub chan_actor_get_roles: unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_pre_net_receive.
    pub chan_actor_pre_net_receive: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_post_net_receive.
    pub chan_actor_post_net_receive: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_post_net_begin_play.
    pub chan_actor_post_net_begin_play: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_should_script_replicate.
    pub chan_actor_should_script_replicate: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_optimized_rep_list.
    pub chan_actor_optimized_rep_list: unsafe extern "C" fn(
        *mut c_void,
        *mut u8,
        *mut c_void,
        *mut i32,
        *mut c_void,
        i32,
        *mut *mut i32,
        *mut i32,
    ) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_process_event.
    pub chan_actor_process_event: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_level_num_reps_add.
    pub chan_actor_level_num_reps_add: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_is_player_pawn.
    pub chan_actor_is_player_pawn: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_handle_client_player.
    pub chan_actor_handle_client_player: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_channels_set.
    pub chan_actor_channels_set: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_remove_sent_temporary.
    pub chan_conn_remove_sent_temporary: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_add_sent_temporary.
    pub chan_conn_add_sent_temporary: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_conn_flush_net.
    pub chan_conn_flush_net: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_driver_recording_demo.
    pub chan_driver_recording_demo: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_map_class_cache.
    pub chan_map_class_cache:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_cache_rep_condition_count.
    pub chan_cache_rep_condition_count: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_cache_max_index.
    pub chan_cache_max_index: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_cache_from_index.
    pub chan_cache_from_index: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_cache_from_field.
    pub chan_cache_from_field:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_cache_rep_property_at.
    pub chan_cache_rep_property_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_field_kind.
    pub chan_field_kind: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_field_name; writes the FName index (i32) to out.
    pub chan_field_name: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_object_init_properties.
    pub chan_object_init_properties: unsafe extern "C" fn(*mut u8, i32, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_class_rep_record.
    pub chan_class_rep_record: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_rep_record_metadata.
    pub chan_rep_record_metadata:
        unsafe extern "C" fn(*mut c_void, *mut *mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_metadata.
    pub chan_property_metadata:
        unsafe extern "C" fn(*mut c_void, *mut i32, *mut i32, *mut i32, *mut i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_replicated_metadata.
    pub chan_property_replicated_metadata:
        unsafe extern "C" fn(*mut c_void, *mut *mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_net_serialize.
    pub chan_property_net_serialize:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut u8, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_copy_single.
    pub chan_property_copy_single: unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_property_destroy_value.
    pub chan_property_destroy_value: unsafe extern "C" fn(*mut c_void, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_property_exit_recent.
    pub chan_property_exit_recent: unsafe extern "C" fn(*mut u8, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_property_identical.
    pub chan_property_identical:
        unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_is_object.
    pub chan_property_is_object: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_name_kind.
    pub chan_property_name_kind: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_is_bool.
    pub chan_property_is_bool: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_property_owner_class.
    pub chan_property_owner_class: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_function_metadata.
    pub chan_function_metadata: unsafe extern "C" fn(
        *mut c_void,
        *mut *mut c_void,
        *mut i32,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> i32,
    /// Narrow C++ leaf adapter for chan_function_find.
    pub chan_function_find: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_function_super.
    pub chan_function_super: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_function_param_first.
    pub chan_function_param_first: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_function_param_next.
    pub chan_function_param_next: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_frame_step_condition.
    pub chan_frame_step_condition:
        unsafe extern "C" fn(*mut c_void, *mut c_void, i32, *mut c_void, *mut u32) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_read_int.
    pub chan_bunch_read_int: unsafe extern "C" fn(*mut FInBunch, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_read_bit.
    pub chan_bunch_read_bit: unsafe extern "C" fn(*mut FInBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_is_error.
    pub chan_bunch_is_error: unsafe extern "C" fn(*mut FInBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_serialize_object.
    pub chan_bunch_serialize_object: unsafe extern "C" fn(*mut FInBunch, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_serialize_vector.
    pub chan_bunch_serialize_vector: unsafe extern "C" fn(*mut FInBunch, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_write_int.
    pub chan_bunch_write_int: unsafe extern "C" fn(*mut FOutBunch, i32, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_serialize_byte.
    pub chan_bunch_serialize_byte: unsafe extern "C" fn(*mut FInBunch, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_write_byte.
    pub chan_bunch_write_byte: unsafe extern "C" fn(*mut FOutBunch, u8) -> i32,
    /// Narrow C++ leaf adapter for chan_bunch_mark_pop.
    pub chan_bunch_mark_pop: unsafe extern "C" fn(*mut FOutBunch, i32, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_get.
    pub chan_retirement_get: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_in_packet.
    pub chan_retirement_in_packet: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_set_in_packet.
    pub chan_retirement_set_in_packet: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_out_packet.
    pub chan_retirement_out_packet: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_set_out.
    pub chan_retirement_set_out: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_reliable.
    pub chan_retirement_reliable: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_set_reliable.
    pub chan_retirement_set_reliable: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_object_to_index.
    pub chan_object_to_index: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_can_serialize_object.
    pub chan_can_serialize_object: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_level_destroy_actor.
    pub chan_level_destroy_actor: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_net_mode_client (reads Actor->Level->NetMode).
    pub chan_actor_net_mode_client: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_download_receive_data.
    pub chan_download_receive_data: unsafe extern "C" fn(*mut c_void, *const u8, i32) -> i32,
    /// Narrow C++ leaf adapter for UDownload::DownloadDone.
    pub chan_download_done: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for delete on UDownload.
    pub chan_download_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_file_archive_serialize.
    pub chan_file_archive_serialize: unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_file_archive_is_error.
    pub chan_file_archive_is_error: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_file_archive_delete.
    pub chan_file_archive_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_package_info.
    pub chan_package_info:
        unsafe extern "C" fn(*mut c_void, i32, *mut i32, *mut *mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_package_find_guid.
    pub chan_package_find_guid: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_package_remove.
    pub chan_package_remove: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_file_create_reader.
    pub chan_file_create_reader: unsafe extern "C" fn(*const u32, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_notify_sending_file.
    pub chan_notify_sending_file: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_string_set_tchar.
    pub chan_string_set_tchar: unsafe extern "C" fn(*mut c_void, *const u32) -> i32,
    /// Narrow C++ leaf adapter for chan_string_append.
    pub chan_string_append: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_describe_format.
    pub chan_actor_describe_format: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_object_is_valid.
    pub chan_object_is_valid: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_object_is_actor.
    pub chan_object_is_actor: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_object_is_class.
    pub chan_object_is_class: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_class_is_actor_subclass.
    pub chan_class_is_actor_subclass: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_player.
    pub chan_actor_player: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Address of the canonical Rust-owned GMem arena; no allocation or callback.
    pub chan_mem_stack: unsafe extern "C" fn() -> *mut FMemStack,
    /// Typed SDK NewZeroed byte/INT views of the existing Rust arena.
    pub chan_mem_new_zeroed: unsafe extern "C" fn(i32, *mut *mut u8) -> i32,
    pub chan_mem_new_int: unsafe extern "C" fn(i32, *mut *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_log_invalid_new_channel_packet.
    pub chan_log_invalid_new_channel_packet: unsafe extern "C" fn(i32, i32, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_log_invalid_actor_class.
    pub chan_log_invalid_actor_class: unsafe extern "C" fn() -> i32,
    /// Narrow C++ leaf adapter for chan_log_actor_not_spawned.
    pub chan_log_actor_not_spawned: unsafe extern "C" fn() -> i32,
    /// Narrow C++ leaf adapter for chan_recent_add.
    pub chan_recent_add: unsafe extern "C" fn(*mut TArrayRaw, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_empty.
    pub chan_retirement_empty: unsafe extern "C" fn(*mut TArrayRaw, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_retirement_add.
    pub chan_retirement_add: unsafe extern "C" fn(*mut TArrayRaw, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_mark_num_bits.
    pub chan_mark_num_bits: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_log_received_unwanted_function.
    pub chan_log_received_unwanted_function: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_log_control_overflow.
    pub chan_log_control_overflow: unsafe extern "C" fn() -> i32,
    /// Narrow C++ leaf adapter for chan_log_file_net_invalid.
    pub chan_log_file_net_invalid: unsafe extern "C" fn() -> i32,
    /// Narrow C++ leaf adapter for chan_log_file_skip.
    pub chan_log_file_skip: unsafe extern "C" fn(*const u32) -> i32,
    /// Narrow C++ leaf adapter for chan_log_file_net_send.
    pub chan_log_file_net_send: unsafe extern "C" fn(*const u32) -> i32,
    /// Narrow C++ leaf adapter for chan_in_bunch_num_bytes.
    pub chan_in_bunch_num_bytes: unsafe extern "C" fn(*mut FInBunch, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_in_bunch_data.
    pub chan_in_bunch_data: unsafe extern "C" fn(*mut FInBunch, *mut *const u8) -> i32,
    /// Narrow C++ leaf adapter for chan_in_bunch_serialize_guid.
    pub chan_in_bunch_serialize_guid: unsafe extern "C" fn(*mut FInBunch, *mut u8) -> i32,
    /// Narrow C++ leaf adapter for chan_string_equals_tchar.
    pub chan_string_equals_tchar: unsafe extern "C" fn(*mut c_void, *const u32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_package_file_info.
    pub chan_package_file_info: unsafe extern "C" fn(*mut c_void, i32, *mut i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_package_has_linker.
    pub chan_package_has_linker: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_file_copy_src_filename.
    pub chan_file_copy_src_filename:
        unsafe extern "C" fn(*mut UFileChannel, *mut c_void, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_serialize_bytes.
    pub chan_out_bunch_serialize_bytes: unsafe extern "C" fn(*mut FOutBunch, *mut u8, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_out_bunch_serialize_vector.
    pub chan_out_bunch_serialize_vector: unsafe extern "C" fn(*mut FOutBunch, *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_actor_level.
    pub chan_actor_level: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// Narrow C++ leaf adapter for chan_player_is_viewport.
    pub chan_player_is_viewport: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    /// Narrow C++ leaf adapter for chan_mem_zero.
    pub chan_mem_zero: unsafe extern "C" fn(*mut u8, i32) -> i32,
    /// Narrow C++ leaf adapter for chan_file_size.
    pub chan_file_size: unsafe extern "C" fn(*const u32, *mut i32) -> i32,
}
const _: () = assert!(std::mem::size_of::<Hp2NetChanServices>() == 165 * std::mem::size_of::<usize>());

macro_rules! call {
    ($e:expr) => {
        if $e != 0 {
            return Err(Thrown);
        }
    };
}
macro_rules! checked {
    ($s:expr,$expr:literal,$line:expr) => {
        call!(($s.chan_check_failed)(
            concat!($expr, "\0").as_ptr().cast(),
            b"UnChan.cpp\0".as_ptr().cast(),
            $line
        ))
    };
}

struct NativeChannelTickOps<'a>(&'a Hp2NetChanServices);

impl ChannelTickOps for NativeChannelTickOps<'_> {
    type Error = Thrown;

    unsafe fn check_failed(
        &self,
        expression: &std::ffi::CStr,
        line: i32,
    ) -> Result<(), Self::Error> {
        unsafe {
            call!((self.0.chan_check_failed)(
                expression.as_ptr(),
                b"UnChan.cpp\0".as_ptr().cast(),
                line,
            ));
            Ok(())
        }
    }

    unsafe fn driver_time_since(
        &self,
        driver: *mut UNetDriver,
        time: *const u8,
    ) -> Result<f32, Self::Error> {
        unsafe {
            let mut seconds = 0.0;
            call!((self.0.chan_driver_time_since)(driver.cast(), time, &mut seconds));
            Ok(seconds)
        }
    }

    unsafe fn send_raw_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe {
            let mut unused = 0;
            call!((self.0.chan_conn_send_raw_bunch)(
                connection.cast(),
                bunch,
                0,
                &mut unused,
            ));
            Ok(())
        }
    }

    unsafe fn connection_is_net_ready(
        &self,
        connection: *mut UNetConnection,
        saturate: i32,
    ) -> Result<i32, Self::Error> {
        unsafe {
            let mut ready = 0;
            call!((self.0.chan_conn_is_net_ready)(connection.cast(), saturate, &mut ready));
            Ok(ready)
        }
    }

    unsafe fn connection_out_num_bits(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<i32, Self::Error> {
        unsafe {
            let mut bits = 0;
            call!((self.0.chan_conn_out_num_bits)(connection.cast(), &mut bits));
            Ok(bits)
        }
    }
}

pub(crate) struct CoreChannelTickOps<'a> {
    access: hp2_core::core_access::CoreAccess<'a>,
}

impl<'a> CoreChannelTickOps<'a> {
    pub(crate) fn new(access: hp2_core::core_access::CoreAccess<'a>) -> Self {
        Self { access }
    }
}

impl ChannelTickOps for CoreChannelTickOps<'_> {
    type Error = hp2_core::core_access::RuntimeError;

    unsafe fn check_failed(
        &self,
        expression: &std::ffi::CStr,
        line: i32,
    ) -> Result<(), Self::Error> {
        Err(hp2_core::core_access::RuntimeError::new(&format!(
            "{} (UnChan.cpp:{line})",
            expression.to_string_lossy()
        )))
    }

    unsafe fn driver_time_since(
        &self,
        driver: *mut UNetDriver,
        time: *const u8,
    ) -> Result<f32, Self::Error> {
        if driver.is_null() || time.is_null() {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Invalid canonical channel timer",
            ));
        }
        let previous = unsafe { ptr::read_unaligned(time.cast::<[u8; 8]>()) };
        Ok(crate::lev_tic::ftime_sub(
            i64::from_ne_bytes(unsafe { (*driver).time }),
            i64::from_ne_bytes(previous),
        ))
    }

    unsafe fn send_raw_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe { crate::connection_owner::send_raw_bunch(self.access, connection, bunch, false)?; }
        Ok(())
    }

    unsafe fn connection_is_net_ready(
        &self,
        connection: *mut UNetConnection,
        saturate: i32,
    ) -> Result<i32, Self::Error> {
        unsafe { crate::connection_owner::is_net_ready(self.access, connection, saturate) }
    }

    unsafe fn connection_out_num_bits(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<i32, Self::Error> {
        if connection.is_null() {
            return Err(hp2_core::core_access::RuntimeError::new(
                "Null canonical UNetConnection",
            ));
        }
        Ok(unsafe {
            hp2_core::bits::writer_get_num_bits(ptr::addr_of_mut!((*connection).out))
        })
    }
}

/// `UChannel::Init`.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_init(ch: *mut UChannel, conn: *mut UNetConnection, index: i32, local: i32) {
    unsafe {
        (*ch).connection = conn.cast();
        (*ch).ch_index = index;
        (*ch).opened_locally = local;
        (*ch).open_packet_id = crate::prim::INDEX_NONE;
        (*ch).negotiated_ver = (*conn).negotiated_ver;
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_set_closing_flag(ch: *mut UChannel) {
    unsafe {
        (*ch).closing = 1;
    }
}
unsafe fn connection_channel_slot(ch: *mut UChannel) -> *mut *mut c_void {
    unsafe {
        let conn = (*ch).connection.cast::<UNetConnection>();
        std::ptr::addr_of_mut!((*conn).channels)
            .cast::<*mut c_void>()
            .offset((*ch).ch_index as isize)
    }
}
unsafe fn connection_reliable_slot(ch: *mut UChannel) -> *mut i32 {
    unsafe {
        let conn = (*ch).connection.cast::<UNetConnection>();
        std::ptr::addr_of_mut!((*conn).in_reliable)
            .cast::<i32>()
            .offset((*ch).ch_index as isize)
    }
}
fn slot_is(ch: *mut UChannel) -> bool {
    unsafe { connection_channel_slot(ch).read() == ch.cast() }
}

/// `UChannel::RouteDestroy`.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_route_destroy(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
) -> Result<i32, Thrown> {
    unsafe {
        let c = (*ch).connection;
        if c.is_null() {
            return Ok(0);
        }
        let mut f = 0;
        call!((s.chan_object_get_flags)(c, &mut f));
        if f & 2 != 0 {
            call!((s.chan_object_clear_flags)(ch.cast(), 0x00800000));
            let mut d = 0;
            call!((s.chan_conn_conditional_destroy)(c, &mut d));
            if d != 0 {
                return Ok(1);
            }
            call!((s.chan_object_set_flags)(ch.cast(), 0x00800000));
        }
        Ok(0)
    }
}
/// `UChannel::Destroy` after a derived route-destroy check.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_destroy(s: &Hp2NetChanServices, ch: *mut UChannel) -> Result<(), Thrown> {
    unsafe {
        if (*ch).connection.is_null() {
            checked!(s, "Connection", 86);
        }
        if !slot_is(ch) {
            checked!(s, "Connection->Channels[ChIndex]==this", 87);
        }
        let mut out = (*ch).out_rec.cast::<FOutBunch>();
        let mut input = (*ch).in_rec.cast::<FInBunch>();
        (*ch).out_rec = std::ptr::null_mut();
        (*ch).in_rec = std::ptr::null_mut();
        (*ch).num_out_rec = 0;
        (*ch).num_in_rec = 0;
        let mut cleanup = Ok(());
        while !out.is_null() {
            let next = (*out).next.cast();
            let result = net_bunch::delete_out(&s.bunches, out);
            cleanup = cleanup.and(result);
            out = next;
        }
        while !input.is_null() {
            let next = (*input).next.cast();
            let result = net_bunch::delete_in(&s.bunches, input);
            cleanup = cleanup.and(result);
            input = next;
        }
        cleanup?;
        let conn = (*ch).connection.cast::<UNetConnection>();
        let mut removed = 0;
        call!((s.chan_tarray_remove_channel)(
            std::ptr::addr_of_mut!((*conn).open_channels),
            ch.cast(),
            &mut removed,
        ));
        if removed != 1 {
            checked!(s, "Connection->OpenChannels.RemoveItem(this)==1", 96);
        }
        connection_channel_slot(ch).write(std::ptr::null_mut());
        (*ch).connection = std::ptr::null_mut();
        call!((s.chan_uobject_destroy)(ch.cast()));
        Ok(())
    }
}
/// `UChannel::Close`.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_close(s: &Hp2NetChanServices, ch: *mut UChannel) -> Result<(), Thrown> {
    unsafe { channel_owner::close_with_ops(&NativeCloseOps(s), ch) }
}

struct NativeCloseOps<'a>(&'a Hp2NetChanServices);

impl CloseOps for NativeCloseOps<'_> {
    type Error = Thrown;

    unsafe fn channel_is_registered(&self, channel: *mut UChannel) -> Result<bool, Thrown> {
        Ok(unsafe { slot_is(channel) })
    }

    unsafe fn connection_state(&self, channel: *mut UChannel) -> Result<i32, Thrown> {
        unsafe {
            let connection = (*channel).connection.cast::<UNetConnection>();
            Ok((*connection).state)
        }
    }

    unsafe fn is_closing(&self, channel: *mut UChannel) -> bool {
        unsafe { (*channel).closing != 0 }
    }

    unsafe fn with_close_bunch<T>(
        &self,
        channel: *mut UChannel,
        body: impl FnOnce(*mut FOutBunch) -> Result<T, Thrown>,
    ) -> Result<T, Thrown> {
        unsafe { net_bunch::with_out(&self.0.bunches, channel, 1, body) }
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Thrown> {
        unsafe {
            let mut error = 0;
            call!((self.0.chan_out_bunch_is_error)(bunch, &mut error));
            Ok(error != 0)
        }
    }

    unsafe fn bunch_is_close(&self, bunch: *mut FOutBunch) -> bool {
        unsafe { (*bunch).b_close != 0 }
    }

    unsafe fn set_reliable(&self, bunch: *mut FOutBunch) {
        unsafe { (*bunch).b_reliable = 1 }
    }

    unsafe fn send_close_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Result<(), Thrown> {
        unsafe { channel_send_bunch(self.0, channel, bunch, 0).map(|_| ()) }
    }

    fn invalid_channel_slot(&self) -> Result<(), Thrown> {
        unsafe { checked!(self.0, "Connection->Channels[ChIndex]==this", 65); }
        Ok(())
    }

    fn invalid_close_bunch(&self) -> Result<(), Thrown> {
        unsafe { checked!(self.0, "!CloseBunch.IsError()", 73); }
        Ok(())
    }

    fn invalid_close_flag(&self) -> Result<(), Thrown> {
        unsafe { checked!(self.0, "CloseBunch.bClose", 74); }
        Ok(())
    }
}
trait SendBunchOps {
    type Error;

    unsafe fn channel_registered(&self, channel: *mut UChannel) -> bool;
    unsafe fn check(&self, expression: &'static [u8], line: i32) -> Result<(), Self::Error>;
    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Self::Error>;
    unsafe fn out_stats(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<(i32, i32, *mut c_void), Self::Error>;
    unsafe fn last_out(&self, connection: *mut UNetConnection) -> Result<*mut FOutBunch, Self::Error>;
    unsafe fn bunch_num_bytes(&self, bunch: *mut FOutBunch) -> Result<i32, Self::Error>;
    unsafe fn append_to_last(
        &self,
        last: *mut FOutBunch,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error>;
    unsafe fn last_out_bunch(&self, connection: *mut UNetConnection) -> Result<*mut FOutBunch, Self::Error>;
    unsafe fn pop_last_start(
        &self,
        connection: *mut UNetConnection,
        end: *mut c_void,
    ) -> Result<(), Self::Error>;
    unsafe fn copy_out(&self, bunch: *mut FOutBunch) -> Result<*mut FOutBunch, Self::Error>;
    unsafe fn assign_out(&self, destination: *mut FOutBunch, source: *mut FOutBunch)
        -> Result<(), Self::Error>;
    unsafe fn set_last_out_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error>;
    unsafe fn send_raw(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<i32, Self::Error>;
    unsafe fn set_closing(&self, channel: *mut UChannel) -> Result<(), Self::Error>;
    unsafe fn set_last_end(&self, connection: *mut UNetConnection) -> Result<(), Self::Error>;
}

unsafe fn send_bunch_with_ops<O: SendBunchOps>(
    ops: &O,
    channel: *mut UChannel,
    mut bunch: *mut FOutBunch,
    mut merge: i32,
) -> Result<i32, O::Error> {
    unsafe {
        if (*channel).closing != 0 {
            ops.check(b"!Closing\0", 303)?;
        }
        if !ops.channel_registered(channel) {
            ops.check(b"Connection->Channels[ChIndex]==this\0", 304)?;
        }
        if ops.bunch_is_error(bunch)? {
            ops.check(b"!Bunch->IsError()\0", 305)?;
        }
        let connection = (*channel).connection.cast::<UNetConnection>();
        if (*channel).open_packet_id == crate::prim::INDEX_NONE && (*channel).opened_locally != 0 {
            (*bunch).b_open = 1;
            (*channel).open_temporary = i32::from((*bunch).b_reliable == 0);
        }
        if (*channel).open_temporary != 0 && (*bunch).b_reliable != 0 {
            ops.check(b"!Bunch->bReliable\0", 316)?;
        }
        let mut out_bunch = ptr::null_mut();
        let (bits, bytes, end) = ops.out_stats(connection)?;
        if merge != 0 && (*connection).allow_merge != 0 {
            let last = ops.last_out(connection)?;
            let bunch_bytes = ops.bunch_num_bytes(bunch)?;
            if (*last).ch_index == (*bunch).ch_index
                && bits != 0
                && bytes + bunch_bytes + 9 <= (*connection).max_packet
            {
                ops.append_to_last(last, bunch)?;
                (*last).b_reliable |= (*bunch).b_reliable;
                (*last).b_open |= (*bunch).b_open;
                (*last).b_close |= (*bunch).b_close;
                out_bunch = ops.last_out_bunch(connection)?;
                bunch = last;
                ops.pop_last_start(connection, end)?;
                (*connection).out_bun_acc = (*connection).out_bun_acc.wrapping_sub(1);
                merge = 1;
            } else {
                merge = 0;
            }
        } else {
            merge = 0;
        }
        if (*bunch).b_reliable != 0 {
            if out_bunch.is_null() {
                if (*channel).num_out_rec >= 127 + (*bunch).b_close as i32 {
                    ops.check(b"NumOutRec<RELIABLE_BUFFER-1+Bunch->bClose\0", 348)?;
                }
                (*bunch).next = ptr::null_mut();
                (*connection).out_reliable[(*channel).ch_index as usize] =
                    (*connection).out_reliable[(*channel).ch_index as usize].wrapping_add(1);
                (*bunch).ch_sequence =
                    (*connection).out_reliable[(*channel).ch_index as usize];
                (*channel).num_out_rec = (*channel).num_out_rec.wrapping_add(1);
                out_bunch = ops.copy_out(bunch)?;
                let mut tail = ptr::addr_of_mut!((*channel).out_rec);
                while !(*tail).is_null() {
                    tail = ptr::addr_of_mut!((*(*tail).cast::<FOutBunch>()).next);
                }
                *tail = out_bunch.cast();
            } else {
                (*bunch).next = (*out_bunch).next;
                ops.assign_out(out_bunch, bunch)?;
            }
            ops.set_last_out_bunch(connection, out_bunch)?;
        } else {
            out_bunch = bunch;
            ops.set_last_out_bunch(connection, ptr::null_mut())?;
        }
        (*out_bunch).received_ack = 0;
        let packet = ops.send_raw(connection, out_bunch)?;
        if (*channel).open_packet_id == crate::prim::INDEX_NONE && (*channel).opened_locally != 0 {
            (*channel).open_packet_id = packet;
        }
        if (*out_bunch).b_close != 0 {
            ops.set_closing(channel)?;
        }
        let last = ops.last_out(connection)?;
        ops.assign_out(last, out_bunch)?;
        ops.set_last_end(connection)?;
        let _ = merge;
        Ok(packet)
    }
}
/// `UChannel::SendBunch`.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_send_bunch(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
    b: *mut FOutBunch,
    merge: i32,
) -> Result<i32, Thrown> {
    unsafe { send_bunch_with_ops(&NativeSendBunchOps(s), ch, b, merge) }
}

struct NativeSendBunchOps<'a>(&'a Hp2NetChanServices);

impl SendBunchOps for NativeSendBunchOps<'_> {
    type Error = Thrown;

    unsafe fn channel_registered(&self, channel: *mut UChannel) -> bool {
        unsafe { slot_is(channel) }
    }

    unsafe fn check(&self, expression: &'static [u8], line: i32) -> Result<(), Thrown> {
        let status = unsafe {
            (self.0.chan_check_failed)(
                expression.as_ptr().cast(),
                b"UnChan.cpp\0".as_ptr().cast(),
                line,
            )
        };
        if status != 0 { Err(Thrown) } else { Ok(()) }
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Thrown> {
        let mut error = 0;
        call!((self.0.chan_out_bunch_is_error)(bunch, &mut error));
        Ok(error != 0)
    }

    unsafe fn out_stats(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<(i32, i32, *mut c_void), Thrown> {
        let mut bits = 0;
        let mut bytes = 0;
        let mut end = ptr::null_mut();
        call!((self.0.chan_conn_out_num_bits)(connection.cast(), &mut bits));
        call!((self.0.chan_conn_out_num_bytes)(connection.cast(), &mut bytes));
        call!((self.0.chan_conn_last_end)(connection.cast(), &mut end));
        Ok((bits, bytes, end))
    }

    unsafe fn last_out(&self, connection: *mut UNetConnection) -> Result<*mut FOutBunch, Thrown> {
        let mut last = ptr::null_mut();
        call!((self.0.chan_conn_last_out)(connection.cast(), &mut last));
        Ok(last)
    }

    unsafe fn bunch_num_bytes(&self, bunch: *mut FOutBunch) -> Result<i32, Thrown> {
        let mut bytes = 0;
        call!((self.0.chan_out_bunch_num_bytes)(bunch, &mut bytes));
        Ok(bytes)
    }

    unsafe fn append_to_last(
        &self,
        last: *mut FOutBunch,
        bunch: *mut FOutBunch,
    ) -> Result<(), Thrown> {
        let mut data = ptr::null();
        let mut bits = 0;
        call!((self.0.chan_out_bunch_data)(bunch, &mut data));
        call!((self.0.chan_out_bunch_num_bits)(bunch, &mut bits));
        call!((self.0.chan_out_bunch_serialize_bits)(last, data, bits));
        Ok(())
    }

    unsafe fn last_out_bunch(&self, connection: *mut UNetConnection) -> Result<*mut FOutBunch, Thrown> {
        let mut bunch = ptr::null_mut();
        call!((self.0.chan_conn_last_out_bunch_get)(connection.cast(), &mut bunch));
        Ok(bunch)
    }

    unsafe fn pop_last_start(
        &self,
        connection: *mut UNetConnection,
        end: *mut c_void,
    ) -> Result<(), Thrown> {
        call!((self.0.chan_mark_pop_last_start)(connection.cast(), end));
        Ok(())
    }

    unsafe fn copy_out(&self, bunch: *mut FOutBunch) -> Result<*mut FOutBunch, Thrown> {
        unsafe { net_bunch::copy_out(&self.0.bunches, bunch) }
    }

    unsafe fn assign_out(&self, destination: *mut FOutBunch, source: *mut FOutBunch)
        -> Result<(), Thrown> {
        unsafe { net_bunch::assign_out(destination, source) }
    }

    unsafe fn set_last_out_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Thrown> {
        call!((self.0.chan_conn_last_out_bunch_set)(connection.cast(), bunch));
        Ok(())
    }

    unsafe fn send_raw(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<i32, Thrown> {
        let mut packet = 0;
        call!((self.0.chan_conn_send_raw_bunch)(connection.cast(), bunch, 1, &mut packet));
        Ok(packet)
    }

    unsafe fn set_closing(&self, channel: *mut UChannel) -> Result<(), Thrown> {
        call!((self.0.chan_virtual_set_closing_flag)(channel.cast()));
        Ok(())
    }

    unsafe fn set_last_end(&self, connection: *mut UNetConnection) -> Result<(), Thrown> {
        call!((self.0.chan_mark_set_last_end)(connection.cast(), connection.cast()));
        Ok(())
    }
}
/// Canonical Core send path for a live channel and Core-allocated bunch.
///
/// # Safety
/// Channel, connection, queues and bunch storage must remain live throughout packet encoding.
pub(crate) unsafe fn channel_send_bunch_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    channel: *mut UChannel,
    bunch: *mut FOutBunch,
    merge: i32,
) -> hp2_core::core_access::RuntimeResult<i32> {
    unsafe { send_bunch_with_ops(&CoreSendBunchOps { access }, channel, bunch, merge) }
}

struct CoreSendBunchOps<'a> {
    access: hp2_core::core_access::CoreAccess<'a>,
}
impl SendBunchOps for CoreSendBunchOps<'_> {
    type Error = hp2_core::core_access::RuntimeError;

    unsafe fn channel_registered(&self, channel: *mut UChannel) -> bool {
        let connection = unsafe { (*channel).connection.cast::<UNetConnection>() };
        let index = unsafe { (*channel).ch_index };
        !connection.is_null()
            && (0..1023).contains(&index)
            && unsafe { (*connection).channels[index as usize] == channel.cast() }
    }

    unsafe fn check(&self, expression: &'static [u8], _line: i32) -> Result<(), Self::Error> {
        let expression = expression.strip_suffix(&[0]).unwrap_or(expression);
        let message = std::str::from_utf8(expression).unwrap_or("Core channel assertion failed");
        Err(hp2_core::core_access::RuntimeError::new(message))
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Self::Error> {
        Ok(unsafe { (*bunch).base.ar_is_error != 0 })
    }

    unsafe fn out_stats(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<(i32, i32, *mut c_void), Self::Error> {
        let bits = unsafe { (*connection).out.num };
        let bytes = bits.wrapping_add(7) >> 3;
        let end = unsafe { ptr::addr_of_mut!((*connection).last_end).cast() };
        Ok((bits, bytes, end))
    }

    unsafe fn last_out(&self, connection: *mut UNetConnection) -> Result<*mut FOutBunch, Self::Error> {
        Ok(unsafe { ptr::addr_of_mut!((*connection).last_out) })
    }

    unsafe fn bunch_num_bytes(&self, bunch: *mut FOutBunch) -> Result<i32, Self::Error> {
        Ok(unsafe { (*bunch).base.num.wrapping_add(7) >> 3 })
    }

    unsafe fn append_to_last(
        &self,
        last: *mut FOutBunch,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe {
            hp2_core::bits::writer_serialize_bits(
                last.cast(),
                (*bunch).base.buffer.data.cast(),
                (*bunch).base.num,
            );
        }
        Ok(())
    }

    unsafe fn last_out_bunch(&self, connection: *mut UNetConnection) -> Result<*mut FOutBunch, Self::Error> {
        Ok(unsafe { (*connection).last_out_bunch.cast() })
    }

    unsafe fn pop_last_start(
        &self,
        connection: *mut UNetConnection,
        _end: *mut c_void,
    ) -> Result<(), Self::Error> {
        let mark = unsafe { (*connection).last_start };
        unsafe {
            hp2_core::bits::writer_mark_pop_with_checker(
                &mut (*connection).out,
                mark.overflowed,
                mark.num,
                |_, _| Err(hp2_core::core_access::RuntimeError::new("Core packet mark pop assertion")),
            )
        }
    }

    unsafe fn copy_out(&self, bunch: *mut FOutBunch) -> Result<*mut FOutBunch, Self::Error> {
        unsafe { net_bunch::copy_out_core(bunch) }
            .map_err(|_| hp2_core::core_access::RuntimeError::new("Core reliable bunch copy failed"))
    }

    unsafe fn assign_out(
        &self,
        destination: *mut FOutBunch,
        source: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe { net_bunch::assign_out_core(destination, source) }
            .map_err(|_| hp2_core::core_access::RuntimeError::new("Core bunch assignment failed"))
    }

    unsafe fn set_last_out_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe { (*connection).last_out_bunch = bunch.cast(); }
        Ok(())
    }

    unsafe fn send_raw(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<i32, Self::Error> {
        unsafe { crate::connection_owner::send_raw_bunch(self.access, connection, bunch, true) }
    }

    unsafe fn set_closing(&self, channel: *mut UChannel) -> Result<(), Self::Error> {
        unsafe { (*channel).closing = 1; }
        Ok(())
    }

    unsafe fn set_last_end(&self, connection: *mut UNetConnection) -> Result<(), Self::Error> {
        unsafe {
            (*connection).last_end.overflowed = (*connection).out.ar_is_error;
            (*connection).last_end.num = (*connection).out.num;
        }
        Ok(())
    }
}
/// `UChannel::MaxSendBytes`: leave framing headroom in the connection packet.
///
/// # Safety
/// `channel` and its connection output state are live.
pub unsafe fn channel_max_send_bytes_with<O: ChannelTickOps>(
    channel: *mut UChannel,
    ops: &O,
) -> Result<i32, O::Error> {
    unsafe {
        let connection = (*channel).connection.cast::<UNetConnection>();
        let bits = ops.connection_out_num_bits(connection)?;
        let header_bits = if bits != 0 { 0 } else { 16 };
        let max_bits = (*connection).max_packet * 8 - header_bits - bits - 65;
        Ok((max_bits / 8).max(0))
    }
}

/// Native adapter for `UChannel::MaxSendBytes`.
///
/// # Safety
/// `channel` and its connection output state are live.
pub unsafe fn channel_max_send_bytes(
    services: &Hp2NetChanServices,
    channel: *mut UChannel,
) -> Result<i32, Thrown> {
    unsafe { channel_max_send_bytes_with(channel, &NativeChannelTickOps(services)) }
}

/// Core adapter for the shared `UChannel::MaxSendBytes` policy.
///
/// # Safety
/// `channel` and its connection output state are live canonical records.
pub unsafe fn channel_max_send_bytes_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    channel: *mut UChannel,
) -> hp2_core::core_access::RuntimeResult<i32> {
    unsafe { channel_max_send_bytes_with(channel, &CoreChannelTickOps::new(access)) }
}

/// `UChannel::IsNetReady`, including the source reliable-bunch saturation limit.
///
/// # Safety
/// `channel` and its connection are live.
pub unsafe fn channel_is_net_ready_with<O: ChannelTickOps>(
    channel: *mut UChannel,
    saturate: i32,
    ops: &O,
) -> Result<i32, O::Error> {
    unsafe {
        if (*channel).num_out_rec >= 127 {
            return Ok(0);
        }
        let connection = (*channel).connection.cast::<UNetConnection>();
        ops.connection_is_net_ready(connection, saturate)
    }
}

/// Native adapter for `UChannel::IsNetReady`.
///
/// # Safety
/// `channel` and its connection are live.
pub unsafe fn channel_is_net_ready(
    services: &Hp2NetChanServices,
    channel: *mut UChannel,
    saturate: i32,
) -> Result<i32, Thrown> {
    unsafe { channel_is_net_ready_with(channel, saturate, &NativeChannelTickOps(services)) }
}

/// Core adapter for `UChannel::IsNetReady`.
///
/// # Safety
/// `channel` and its connection are live canonical records.
pub unsafe fn channel_is_net_ready_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    channel: *mut UChannel,
    saturate: i32,
) -> hp2_core::core_access::RuntimeResult<i32> {
    unsafe { channel_is_net_ready_with(channel, saturate, &CoreChannelTickOps::new(access)) }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_is_known_type(s: &Hp2NetChanServices, ty: i32) -> Result<i32, Thrown> {
    unsafe {
        if !(0..8).contains(&ty) {
            return Ok(0);
        }
        let mut c = std::ptr::null_mut();
        call!((s.chan_channel_class_at)(ty, &mut c));
        Ok((!c.is_null()) as i32)
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
/// Operations required by the shared incoming-channel sequencing policy.
pub(crate) trait ChannelReceiveOps {
    type Error;

    unsafe fn check_fail(&self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn copy_in(&self, bunch: *const FInBunch) -> Result<*mut FInBunch, Self::Error>;
    unsafe fn delete_in(&self, bunch: *mut FInBunch) -> Result<(), Self::Error>;
    unsafe fn received_bunch(&self, channel: *mut UChannel, bunch: *mut FInBunch) -> Result<(), Self::Error>;
    unsafe fn delete_channel(&self, channel: *mut UChannel) -> Result<(), Self::Error>;
    unsafe fn delete_out(&self, bunch: *mut FOutBunch) -> Result<(), Self::Error>;
    unsafe fn send_raw_bunch(&self, channel: *mut UChannel, bunch: *mut FOutBunch) -> Result<(), Self::Error>;
}

struct NativeChannelReceiveOps<'a>(&'a Hp2NetChanServices);

impl ChannelReceiveOps for NativeChannelReceiveOps<'_> {
    type Error = Thrown;

    unsafe fn check_fail(&self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Thrown> {
        unsafe {
            call!((self.0.chan_check_failed)(expression.as_ptr(), c"UnChan.cpp".as_ptr(), line));
        }
        Ok(())
    }
    unsafe fn copy_in(&self, bunch: *const FInBunch) -> Result<*mut FInBunch, Thrown> {
        unsafe { net_bunch::copy_in(&self.0.bunches, bunch) }
    }
    unsafe fn delete_in(&self, bunch: *mut FInBunch) -> Result<(), Thrown> {
        unsafe { net_bunch::delete_in(&self.0.bunches, bunch) }
    }
    unsafe fn received_bunch(&self, channel: *mut UChannel, bunch: *mut FInBunch) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_virtual_received_bunch)(channel.cast(), bunch)); }
        Ok(())
    }
    unsafe fn delete_channel(&self, channel: *mut UChannel) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_channel_delete)(channel.cast())); }
        Ok(())
    }
    unsafe fn delete_out(&self, bunch: *mut FOutBunch) -> Result<(), Thrown> {
        unsafe { net_bunch::delete_out(&self.0.bunches, bunch) }
    }
    unsafe fn send_raw_bunch(&self, channel: *mut UChannel, bunch: *mut FOutBunch) -> Result<(), Thrown> {
        let mut unused = 0;
        unsafe {
            call!((self.0.chan_conn_send_raw_bunch)(
                (*channel).connection,
                bunch,
                0,
                &mut unused,
            ));
        }
        Ok(())
    }
}

pub unsafe fn channel_received_sequenced(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
    bunch: *mut FInBunch,
) -> Result<i32, Thrown> {
    unsafe { channel_received_sequenced_with(&NativeChannelReceiveOps(s), ch, bunch).map(i32::from) }
}

pub(crate) unsafe fn channel_received_sequenced_with<O: ChannelReceiveOps>(
    ops: &O,
    ch: *mut UChannel,
    bunch: *mut FInBunch,
) -> Result<bool, O::Error> {
    unsafe {
        if (*bunch).b_reliable != 0 {
            connection_reliable_slot(ch).write((*bunch).ch_sequence);
        }
        if (*ch).closing == 0 {
            ops.received_bunch(ch, bunch)?;
        }
        if (*bunch).b_close != 0 {
            ops.delete_channel(ch)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

pub unsafe fn channel_assert_in_sequenced(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
) -> Result<(), Thrown> {
    unsafe { channel_assert_in_sequenced_with(&NativeChannelReceiveOps(s), ch) }
}

pub(crate) unsafe fn channel_assert_in_sequenced_with<O: ChannelReceiveOps>(
    ops: &O,
    ch: *mut UChannel,
) -> Result<(), O::Error> {
    unsafe {
        let mut incoming = (*ch).in_rec.cast::<FInBunch>();
        while !incoming.is_null() && !(*incoming).next.is_null() {
            let next = (*incoming).next.cast::<FInBunch>();
            if (*next).ch_sequence <= (*incoming).ch_sequence {
                ops.check_fail(c"In->Next->ChSequence>In->ChSequence", 190)?;
            }
            incoming = next;
        }
    }
    Ok(())
}

pub unsafe fn channel_received_raw(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
    bunch: *mut FInBunch,
) -> Result<(), Thrown> {
    unsafe { channel_received_raw_with(&NativeChannelReceiveOps(s), ch, bunch) }
}

pub(crate) unsafe fn channel_received_raw_with<O: ChannelReceiveOps>(
    ops: &O,
    ch: *mut UChannel,
    bunch: *mut FInBunch,
) -> Result<(), O::Error> {
    unsafe {
        if !slot_is(ch) {
            ops.check_fail(c"Connection->Channels[ChIndex]==this", 231)?;
        }
        let reliable = connection_reliable_slot(ch);
        if (*bunch).b_reliable != 0 && (*bunch).ch_sequence != reliable.read().wrapping_add(1) {
            if (*bunch).ch_sequence <= reliable.read() {
                ops.check_fail(c"Bunch.ChSequence > Connection->InReliable[ChIndex]", 241)?;
            }
            let mut position = std::ptr::addr_of_mut!((*ch).in_rec);
            while !(*position).is_null() {
                let queued = (*position).cast::<FInBunch>();
                if (*queued).ch_sequence == (*bunch).ch_sequence {
                    return Ok(());
                }
                if (*queued).ch_sequence > (*bunch).ch_sequence {
                    break;
                }
                position = std::ptr::addr_of_mut!((*queued).next);
            }
            let queued = ops.copy_in(bunch)?;
            (*queued).next = *position;
            *position = queued.cast();
            (*ch).num_in_rec += 1;
            if (*ch).num_in_rec > 128 {
                ops.check_fail(c"NumInRec<=RELIABLE_BUFFER", 263)?;
            }
            channel_assert_in_sequenced_with(ops, ch)?;
            return Ok(());
        }
        if channel_received_sequenced_with(ops, ch, bunch)? {
            return Ok(());
        }
        while !(*ch).in_rec.is_null() {
            let queued = (*ch).in_rec.cast::<FInBunch>();
            if (*queued).ch_sequence != reliable.read().wrapping_add(1) {
                break;
            }
            (*ch).in_rec = (*queued).next;
            (*ch).num_in_rec -= 1;
            let result = channel_received_sequenced_with(ops, ch, queued);
            let cleanup = ops.delete_in(queued);
            let deleted = result?;
            cleanup?;
            if deleted {
                return Ok(());
            }
            channel_assert_in_sequenced_with(ops, ch)?;
        }
    }
    Ok(())
}

pub unsafe fn channel_received_acks(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
) -> Result<(), Thrown> {
    unsafe { channel_received_acks_with(&NativeChannelReceiveOps(s), ch) }
}

pub(crate) unsafe fn channel_received_acks_with<O: ChannelReceiveOps>(
    ops: &O,
    ch: *mut UChannel,
) -> Result<(), O::Error> {
    unsafe {
        if !slot_is(ch) {
            ops.check_fail(c"Connection->Channels[ChIndex]==this", 110)?;
        }
        let mut verify = (*ch).out_rec.cast::<FOutBunch>();
        while !verify.is_null() && !(*verify).next.is_null() {
            let next = (*verify).next.cast::<FOutBunch>();
            if (*next).ch_sequence <= (*verify).ch_sequence {
                ops.check_fail(c"Out->Next->ChSequence>Out->ChSequence", 114)?;
            }
            verify = next;
        }
        let mut close = 0;
        while !(*ch).out_rec.is_null() && (*(*ch).out_rec.cast::<FOutBunch>()).received_ack != 0 {
            let out = (*ch).out_rec.cast::<FOutBunch>();
            close |= (*out).b_close;
            (*ch).out_rec = (*out).next;
            ops.delete_out(out)?;
            (*ch).num_out_rec -= 1;
        }
        if close != 0 || ((*ch).open_temporary != 0 && (*ch).open_acked != 0) {
            if !(*ch).out_rec.is_null() {
                ops.check_fail(c"!OutRec", 130)?;
            }
            ops.delete_channel(ch)?;
        }
    }
    Ok(())
}

pub(crate) unsafe fn channel_received_nak_with<O: ChannelReceiveOps>(
    ops: &O,
    ch: *mut UChannel,
    id: i32,
) -> Result<(), O::Error> {
    unsafe {
        let mut out = (*ch).out_rec.cast::<FOutBunch>();
        while !out.is_null() {
            if (*out).packet_id == id && (*out).received_ack == 0 {
                if (*out).b_reliable == 0 {
                    ops.check_fail(c"Out->bReliable", 432)?;
                }
                ops.send_raw_bunch(ch, out)?;
            }
            out = (*out).next.cast();
        }
    }
    Ok(())
}
/// Shared `UChannel::Tick` body used by native and canonical channel owners.
///
/// # Safety
/// `channel` and its reliable resend chain are live.
pub unsafe fn channel_tick_with<O: ChannelTickOps>(
    channel: *mut UChannel,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        if (*channel).ch_index == 0 && (*channel).open_acked == 0 {
            let connection = (*channel).connection.cast::<UNetConnection>();
            let mut bunch = (*channel).out_rec.cast::<FOutBunch>();
            while !bunch.is_null() {
                if (*bunch).received_ack == 0 {
                    let elapsed = ops.driver_time_since(
                        (*connection).driver.cast::<UNetDriver>(),
                        (*bunch).time.as_ptr(),
                    )?;
                    if elapsed > 1.0 {
                        if (*bunch).b_reliable == 0 {
                            ops.check_failed(c"Out->bReliable", 172)?;
                        }
                        ops.send_raw_bunch(connection, bunch)?;
                    }
                }
                bunch = (*bunch).next.cast();
            }
        }
    }
    Ok(())
}

/// Native adapter for the shared `UChannel::Tick` body.
///
/// # Safety
/// `channel` and its reliable resend chain are live.
pub unsafe fn channel_tick(
    services: &Hp2NetChanServices,
    channel: *mut UChannel,
) -> Result<(), Thrown> {
    unsafe { channel_tick_with(channel, &NativeChannelTickOps(services)) }
}

/// Core adapter for the shared `UChannel::Tick` body.
///
/// # Safety
/// `channel` and its reliable resend chain are live canonical records.
pub unsafe fn channel_tick_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    channel: *mut UChannel,
) -> hp2_core::core_access::RuntimeResult<()> {
    unsafe { channel_tick_with(channel, &CoreChannelTickOps::new(access)) }
}
pub unsafe fn channel_received_nak(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
    id: i32,
) -> Result<(), Thrown> {
    unsafe { channel_received_nak_with(&NativeChannelReceiveOps(s), ch, id) }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn channel_describe(
    s: &Hp2NetChanServices,
    ch: *mut UChannel,
    out: *mut c_void,
) -> Result<(), Thrown> {
    unsafe { describe_base_with(&mut NativeDescriptionOps { services: s, output: out }, ch) }
}

/// `UControlChannel::Destroy`: honor RouteDestroy before the base teardown.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn control_destroy(s: &Hp2NetChanServices, ch: *mut c_void) -> Result<(), Thrown> {
    unsafe {
        if channel_route_destroy(s, ch.cast())? == 0 {
            channel_destroy(s, ch.cast())?;
        }
        Ok(())
    }
}

/// Shared `UControlChannel::ReceivedBunch` text loop.
pub(crate) trait ControlReceiveOps {
    type Error;
    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    /// Reads one text record, notifies it when valid, and returns true on archive error.
    unsafe fn receive_text(&mut self, channel: *mut UChannel, bunch: *mut FInBunch) -> Result<bool, Self::Error>;
}

/// `UControlChannel::ReceivedBunch`: deserialize and notify until archive error.
///
/// # Safety
/// `channel` and `bunch` must be live and the bunch must retain its packet reader.
pub(crate) unsafe fn control_received_bunch_with<O: ControlReceiveOps>(
    ops: &mut O,
    channel: *mut UChannel,
    bunch: *mut FInBunch,
) -> Result<(), O::Error> {
    unsafe {
        if (*channel).closing != 0 {
            ops.check_fail(c"!Closing", 481)?;
        }
        while !ops.receive_text(channel, bunch)? {}
    }
    Ok(())
}

struct NativeControlReceiveOps<'a>(&'a Hp2NetChanServices);

impl ControlReceiveOps for NativeControlReceiveOps<'_> {
    type Error = Thrown;

    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Thrown> {
        unsafe {
            call!((self.0.chan_check_failed)(
                expression.as_ptr(),
                c"UnChan.cpp".as_ptr(),
                line,
            ));
        }
        Ok(())
    }
    unsafe fn receive_text(&mut self, channel: *mut UChannel, bunch: *mut FInBunch) -> Result<bool, Thrown> {
        let services = self.0;
        unsafe {
            net_bunch::with_string(|text| {
                call!((services.chan_in_bunch_serialize_string)(bunch, text));
                let mut error = 0;
                call!((services.chan_in_bunch_is_error)(bunch, &mut error));
                if error != 0 {
                    return Ok(true);
                }
                call!((services.chan_notify_received_text)((*channel).connection, text));
                Ok(false)
            })
        }
    }
}

pub unsafe fn control_received_bunch(
    s: &Hp2NetChanServices,
    ch: *mut c_void,
    bunch: *mut FInBunch,
) -> Result<(), Thrown> {
    unsafe {
        control_received_bunch_with(
            &mut NativeControlReceiveOps(s),
            ch.cast(),
            bunch,
        )
    }
}
struct CoreControlReceiveOps<'a> {
    access: hp2_core::core_access::CoreAccess<'a>,
}

impl ControlReceiveOps for CoreControlReceiveOps<'_> {
    type Error = hp2_core::core_access::RuntimeError;

    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> hp2_core::core_access::RuntimeResult<()> {
        Err(hp2_core::core_access::RuntimeError::new(&format!(
            "ControlChannel invariant failed at UnChan.cpp:{line}: {}",
            expression.to_string_lossy()
        )))
    }
    unsafe fn receive_text(&mut self, channel: *mut UChannel, bunch: *mut FInBunch) -> hp2_core::core_access::RuntimeResult<bool> {
        let mut text = Vec::new();
        let decoded = unsafe {
            crate::actor_channel_owner::with_core_bunch_archive(self.access, bunch, |archive| {
                archive.serialize_fstring(&mut text)
            })
        };
        if unsafe { (*bunch).base.ar_is_error != 0 } {
            return Ok(true);
        }
        decoded?;
        text.push(0);
        unsafe {
            let connection = (*channel).connection.cast::<UNetConnection>();
            crate::level_net_owner::notify_received_text_for_driver(
                self.access,
                (*connection).driver,
                connection,
                text.as_ptr(),
            )?;
        }
        Ok(false)
    }
}

/// Core-owned counterpart to `control_received_bunch`; no native service table is used.
///
/// # Safety
/// `channel` and `bunch` must be live and the bunch must retain its packet reader.
pub unsafe fn control_received_bunch_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    channel: *mut UChannel,
    bunch: *mut FInBunch,
) -> hp2_core::core_access::RuntimeResult<()> {
    unsafe { control_received_bunch_with(&mut CoreControlReceiveOps { access }, channel, bunch) }
}


/// Operations that adapt native or Core-owned control text serialization.
pub(crate) trait ControlSendOps {
    type Error;

    unsafe fn serialize_text(
        &self,
        bunch: *mut FOutBunch,
        text: *const u32,
    ) -> Result<(), Self::Error>;
    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Self::Error>;
    fn log_overflow(&self) -> Result<(), Self::Error>;
    unsafe fn send(&self, channel: *mut UChannel, bunch: *mut FOutBunch)
        -> Result<(), Self::Error>;
}

pub(crate) unsafe fn control_send_with_ops<O: ControlSendOps>(
    ops: &O,
    channel: *mut UChannel,
    bunch: *mut FOutBunch,
    text: *const u32,
) -> Result<(), O::Error> {
    unsafe {
        (*bunch).b_reliable = 1;
        ops.serialize_text(bunch, text)?;
        if ops.bunch_is_error(bunch)? {
            ops.log_overflow()
        } else {
            ops.send(channel, bunch)
        }
    }
}
/// `UControlChannel::SendText`: serialize, overflow-log, or reliably send one message.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn control_serialize(
    s: &Hp2NetChanServices,
    ch: *mut c_void,
    data: *const u32,
    _msg: i32,
) -> Result<(), Thrown> {
    unsafe {
        net_bunch::with_out(&s.bunches, ch.cast(), 0, |bunch| {
            control_send_with_ops(&NativeControlSendOps(s), ch.cast(), bunch, data)
        })
    }
}

struct NativeControlSendOps<'a>(&'a Hp2NetChanServices);

impl ControlSendOps for NativeControlSendOps<'_> {
    type Error = Thrown;

    unsafe fn serialize_text(
        &self,
        bunch: *mut FOutBunch,
        text: *const u32,
    ) -> Result<(), Thrown> {
        net_bunch::with_string(|string| {
            net_bunch::assign_string(string, text)?;
            call!((self.0.chan_out_bunch_serialize_string)(bunch, string));
            Ok(())
        })
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Thrown> {
        let mut error = 0;
        call!((self.0.chan_out_bunch_is_error)(bunch, &mut error));
        Ok(error != 0)
    }

    fn log_overflow(&self) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_log_control_overflow)()); }
        Ok(())
    }

    unsafe fn send(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Result<(), Thrown> {
        unsafe { channel_send_bunch(self.0, channel, bunch, 1).map(|_| ()) }
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn control_describe(
    s: &Hp2NetChanServices,
    ch: *mut c_void,
    out: *mut c_void,
) -> Result<(), Thrown> {
    unsafe { describe_control_with(&mut NativeDescriptionOps { services: s, output: out }, ch.cast()) }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_init(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
    conn: *mut UNetConnection,
    index: i32,
    local: i32,
) -> Result<(), Thrown> {
    unsafe {
        channel_init(ch.cast(), conn, index, local);
        call!((s.chan_driver_notify_get_level)(
            (*conn).driver,
            &mut (*ch).level
        ));
        call!((s.chan_driver_time_into)(
            (*conn).driver,
            (*ch).relevant_time.as_mut_ptr()
        ));
        call!((s.chan_driver_time_minus_spawn_priority)(
            (*conn).driver,
            (*ch).last_update_time.as_mut_ptr()
        ));
        (*ch).num_reps = 0;
        Ok(())
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_set_closing(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
) -> Result<(), Thrown> {
    unsafe {
        if !(*ch).actor.is_null() {
            let c = (*ch.cast::<UChannel>()).connection;
            call!((s.chan_conn_actor_channels_remove)(c, (*ch).actor));
        }
        channel_set_closing_flag(ch.cast());
        Ok(())
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_close(s: &Hp2NetChanServices, ch: *mut UActorChannel) -> Result<(), Thrown> {
    unsafe {
        channel_close(s, ch.cast())?;
        (*ch).actor = std::ptr::null_mut();
    }
    Ok(())
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_tick(s: &Hp2NetChanServices, ch: *mut UActorChannel) -> Result<(), Thrown> {
    unsafe { channel_tick(s, ch.cast()) }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_destroy(s: &Hp2NetChanServices, ch: *mut UActorChannel) -> Result<(), Thrown> {
    unsafe {
        let base = ch.cast::<UChannel>();
        let conn = (*base).connection;
        if conn.is_null() {
            checked!(s, "Connection", 588);
        }
        if channel_route_destroy(s, base)? != 0 {
            return Ok(());
        }
        let slots =
            std::ptr::addr_of!((*conn.cast::<UNetConnection>()).channels).cast::<*mut c_void>();
        if *slots.offset((*base).ch_index as isize) != ch.cast() {
            checked!(s, "Connection->Channels[ChIndex]==this", 591);
        }
        actor_set_closing(s, ch)?;

        let driver = (*conn.cast::<UNetConnection>()).driver;
        let server = (*driver.cast::<UNetDriver>()).server_connection;
        if (*ch).recent.num != 0 {
            if (*ch).actor_class.is_null() {
                checked!(s, "ActorClass", 599);
            }
            call!((s.chan_property_exit_recent)(
                (*ch).recent.data.cast(),
                (*ch).actor_class,
            ));
        }

        if !server.is_null() {
            if !(*ch).actor.is_null() {
                let mut valid = 0;
                call!((s.chan_object_is_valid)((*ch).actor, &mut valid));
                if valid == 0 {
                    checked!(s, "Actor->IsValid()", 612);
                }
            }
            if (*ch).level.is_null() {
                checked!(s, "Level", 613);
            }
            let mut valid = 0;
            call!((s.chan_object_is_valid)((*ch).level, &mut valid));
            if valid == 0 {
                checked!(s, "Level->IsValid()", 614);
            }
            if conn.is_null() {
                checked!(s, "Connection", 615);
            }
            call!((s.chan_object_is_valid)(conn, &mut valid));
            if valid == 0 {
                checked!(s, "Connection->IsValid()", 616);
            }
            if driver.is_null() {
                checked!(s, "Connection->Driver", 617);
            }
            call!((s.chan_object_is_valid)(driver, &mut valid));
            if valid == 0 {
                checked!(s, "Connection->Driver->IsValid()", 618);
            }
            if !(*ch).actor.is_null() {
                let mut temporary = 0;
                let mut is_static = 0;
                let mut no_delete = 0;
                call!((s.chan_actor_flags)(
                    (*ch).actor,
                    &mut valid,
                    &mut temporary,
                    &mut is_static,
                    &mut no_delete,
                ));
                if temporary == 0 {
                    let mut actor_level = std::ptr::null_mut();
                    call!((s.chan_actor_level)((*ch).actor, &mut actor_level));
                    call!((s.chan_level_destroy_actor)(actor_level, (*ch).actor, 1));
                }
            }
        } else if !(*ch).actor.is_null() && (*base).open_acked == 0 {
            call!((s.chan_conn_remove_sent_temporary)(conn, (*ch).actor));
        }
        channel_destroy(s, base)?;
        Ok(())
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_received_nak(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
    id: i32,
) -> Result<(), Thrown> {
    unsafe {
        channel_received_nak(s, ch.cast(), id)?;
        if !(*ch).actor_class.is_null() {
            let mut count = 0;
            call!((s.chan_array_num)(
                std::ptr::addr_of!((*ch).retirement),
                &mut count
            ));
            for i in (0..count).rev() {
                let mut retirement = std::ptr::null_mut();
                call!((s.chan_retirement_get)(
                    std::ptr::addr_of_mut!((*ch).retirement).cast(),
                    i,
                    &mut retirement,
                ));
                let mut packet = 0;
                let mut reliable = 0;
                call!((s.chan_retirement_out_packet)(retirement, &mut packet));
                call!((s.chan_retirement_reliable)(retirement, &mut reliable));
                if packet == id && reliable == 0 {
                    let mut inserted = 0;
                    call!((s.chan_array_add_unique_i32)(
                        std::ptr::addr_of_mut!((*ch).dirty),
                        i,
                        &mut inserted,
                    ));
                }
            }
        }
        Ok(())
    }
}

///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
/// Property metadata used by the shared ActorChannel receive policies.
#[derive(Clone, Copy)]
pub(crate) struct ActorPropertyMetadata {
    pub offset: i32,
    pub element_size: i32,
    pub array_dim: i32,
    pub index: i32,
    pub flags: i32,
}

/// Operations shared by native and canonical actor-channel receive policy.
pub(crate) trait ActorSetActorOps {
    type Error;
    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn actor_class(&mut self, actor: *mut c_void) -> Result<*mut UClass, Self::Error>;
    unsafe fn class_net_cache(&mut self, package_map: *mut UPackageMap, class: *mut UClass) -> Result<*mut FClassNetCache, Self::Error>;
    unsafe fn actor_channel_set(&mut self, connection: *mut UNetConnection, actor: *mut c_void, channel: *mut UActorChannel) -> Result<(), Self::Error>;
    unsafe fn rep_eval_add_zeroed(&mut self, channel: *mut UActorChannel, count: i32) -> Result<(), Self::Error>;
    unsafe fn actor_flags(&mut self, actor: *mut c_void) -> Result<(i32, i32, i32, i32), Self::Error>;
    unsafe fn recent_add(&mut self, channel: *mut UActorChannel, size: i32) -> Result<*mut u8, Self::Error>;
    unsafe fn object_init_properties(&mut self, data: *mut u8, size: i32, class: *mut UClass) -> Result<(), Self::Error>;
    unsafe fn property_metadata(&mut self, property: *mut UProperty) -> Result<ActorPropertyMetadata, Self::Error>;
    unsafe fn property_bool_mask(&mut self, property: *mut UProperty) -> Result<u32, Self::Error>;
    unsafe fn property_is_bool(&mut self, property: *mut UProperty) -> Result<bool, Self::Error>;
    unsafe fn property_destroy_value(&mut self, property: *mut UProperty, data: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn mem_zero(&mut self, data: *mut u8, size: i32) -> Result<(), Self::Error>;
    unsafe fn retirement_empty(&mut self, channel: *mut UActorChannel, count: i32) -> Result<(), Self::Error>;
    unsafe fn retirement_num(&mut self, channel: *mut UActorChannel) -> Result<i32, Self::Error>;
    unsafe fn retirement_add(&mut self, channel: *mut UActorChannel) -> Result<(), Self::Error>;
}

/// Engine-specific leaves needed by the common `UActorChannel::ReceivedBunch`
/// body. CoreAccess is held by its Core implementation, never passed to native.
pub(crate) trait ActorReceiveOps: ActorSetActorOps {
    unsafe fn invalid_new_channel_packet(&mut self, bunch: *mut FInBunch) -> Result<(), Self::Error>;
    unsafe fn serialize_object(&mut self, bunch: *mut FInBunch) -> Result<*mut c_void, Self::Error>;
    unsafe fn object_is_actor(&mut self, object: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn object_is_class(&mut self, object: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn class_is_actor_subclass(&mut self, class: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn log_invalid_actor_class(&mut self) -> Result<(), Self::Error>;
    unsafe fn serialize_vector(&mut self, bunch: *mut FInBunch) -> Result<FVector, Self::Error>;
    unsafe fn spawn_actor(&mut self, level: *mut c_void, class: *mut c_void, location: FVector) -> Result<*mut c_void, Self::Error>;
    unsafe fn log_actor_not_spawned(&mut self) -> Result<(), Self::Error>;
    unsafe fn actor_pre_net_receive(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn actor_post_net_receive(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn actor_post_net_begin_play(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn actor_is_player_pawn(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn actor_player(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn player_is_viewport(&mut self, player: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn cache_max_index(&mut self, cache: *mut FClassNetCache) -> Result<i32, Self::Error>;
    unsafe fn bunch_read_int(&mut self, bunch: *mut FInBunch, max_index: i32) -> Result<i32, Self::Error>;
    unsafe fn bunch_is_error(&mut self, bunch: *mut FInBunch) -> Result<bool, Self::Error>;
    unsafe fn cache_from_index(&mut self, cache: *mut FClassNetCache, index: i32) -> Result<*mut FFieldNetCache, Self::Error>;
    unsafe fn field_kind(&mut self, field: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn property_owner_class(&mut self, property: *mut UProperty) -> Result<*mut UClass, Self::Error>;
    unsafe fn actor_roles(&mut self, actor: *mut c_void) -> Result<(u8, u8), Self::Error>;
    unsafe fn frame_step_condition(&mut self, actor: *mut c_void, owner_class: *mut UClass, rep_offset: i32) -> Result<u32, Self::Error>;
    unsafe fn retirement_get(&mut self, channel: *mut UActorChannel, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn retirement_in_packet(&mut self, retirement: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn retirement_set_in_packet(&mut self, retirement: *mut c_void, packet: i32) -> Result<(), Self::Error>;
    unsafe fn with_network_mem_scope<T>(&mut self, body: impl FnOnce(&mut Self) -> Result<T, Self::Error>) -> Result<T, Self::Error>;
    unsafe fn mem_new_zeroed(&mut self, size: i32) -> Result<*mut u8, Self::Error>;
    unsafe fn property_net_serialize(&mut self, property: *mut UProperty, bunch: *mut FInBunch, package_map: *mut UPackageMap, data: *mut u8) -> Result<(), Self::Error>;
    unsafe fn property_copy_single(&mut self, property: *mut UProperty, destination: *mut u8, source: *mut u8) -> Result<(), Self::Error>;
    unsafe fn bunch_serialize_byte(&mut self, bunch: *mut FInBunch) -> Result<u8, Self::Error>;
    unsafe fn field_name(&mut self, field: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn function_find(&mut self, actor: *mut c_void, name: i32) -> Result<*mut UFunction, Self::Error>;
    unsafe fn function_super(&mut self, function: *mut UFunction) -> Result<*mut UFunction, Self::Error>;
    unsafe fn function_metadata(&mut self, function: *mut UFunction) -> Result<(*mut UClass, i32), Self::Error>;
    unsafe fn log_received_unwanted_function(&mut self, name: i32, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn function_param_first(&mut self, function: *mut UFunction) -> Result<*mut UProperty, Self::Error>;
    unsafe fn function_param_next(&mut self, parameter: *mut UProperty) -> Result<*mut UProperty, Self::Error>;
    unsafe fn object_to_index(&mut self, package_map: *mut UPackageMap, object: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn bunch_read_bit(&mut self, bunch: *mut FInBunch) -> Result<bool, Self::Error>;
    unsafe fn actor_set_client_demo_net_func(&mut self, actor: *mut c_void, value: bool) -> Result<(), Self::Error>;
    unsafe fn actor_process_event(&mut self, actor: *mut c_void, function: *mut UFunction, parameters: *mut u8) -> Result<(), Self::Error>;
    unsafe fn actor_handle_client_player(&mut self, connection: *mut UNetConnection, actor: *mut c_void) -> Result<(), Self::Error>;
}

/// Operations shared by native and canonical actor-channel send policy.
pub(crate) trait ActorReplicateOps: ActorSetActorOps {
    unsafe fn with_out_bunch<T>(
        &mut self,
        channel: *mut UChannel,
        close: i32,
        body: impl FnOnce(&mut Self, *mut FOutBunch) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    unsafe fn with_mem_scope<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    unsafe fn out_bunch_is_error(&mut self, bunch: *mut FOutBunch) -> Result<bool, Self::Error>;
    unsafe fn array_num(&mut self, array: *const TArrayRaw) -> Result<i32, Self::Error>;
    unsafe fn retirement_get(&mut self, channel: *mut UActorChannel, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn retirement_out_packet(&mut self, retirement: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn retirement_reliable(&mut self, retirement: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn array_add_unique_i32(&mut self, array: *mut TArrayRaw, value: i32) -> Result<(), Self::Error>;
    unsafe fn actor_is_player_pawn(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn actor_player(&mut self, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn player_is_viewport(&mut self, player: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn driver_recording_demo(&mut self, driver: *mut UNetDriver) -> Result<bool, Self::Error>;
    unsafe fn actor_net_mode_client(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn pm_serialize_object(
        &mut self,
        map: *mut UPackageMap,
        bunch: *mut FOutBunch,
        object: *mut *mut c_void,
    ) -> Result<bool, Self::Error>;
    unsafe fn out_bunch_serialize_vector(&mut self, bunch: *mut FOutBunch, vector: *mut FVector) -> Result<(), Self::Error>;
    unsafe fn actor_roles(&mut self, actor: *mut c_void) -> Result<(u8, u8), Self::Error>;
    unsafe fn mem_new_int(&mut self, count: i32) -> Result<*mut i32, Self::Error>;
    unsafe fn actor_optimized_rep_list(
        &mut self,
        actor: *mut c_void,
        compare: *mut u8,
        retirement: *mut c_void,
        reps: *mut i32,
        map: *mut UPackageMap,
        num_reps: i32,
    ) -> Result<(*mut i32, i32), Self::Error>;
    unsafe fn actor_should_script_replicate(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn cache_rep_property_at(&mut self, cache: *mut FClassNetCache, index: i32) -> Result<*mut FFieldNetCache, Self::Error>;
    unsafe fn property_is_object(&mut self, property: *mut UProperty) -> Result<bool, Self::Error>;
    unsafe fn can_serialize_object(&mut self, map: *mut UPackageMap, object_value: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn property_identical(&mut self, property: *mut UProperty, a: *mut u8, b: *mut u8) -> Result<bool, Self::Error>;
    unsafe fn property_replicated_metadata(&mut self, property: *mut UProperty) -> Result<(*mut c_void, i32), Self::Error>;
    unsafe fn property_owner_class(&mut self, property: *mut UProperty) -> Result<*mut UClass, Self::Error>;
    unsafe fn frame_step_condition(&mut self, actor: *mut c_void, owner_class: *mut UClass, offset: i32) -> Result<u32, Self::Error>;
    unsafe fn class_rep_record(&mut self, class: *mut UClass, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn rep_record_metadata(&mut self, record: *mut c_void) -> Result<(*mut UProperty, i32), Self::Error>;
    unsafe fn property_name_kind(&mut self, property: *mut UProperty) -> Result<i32, Self::Error>;
    unsafe fn cache_from_field(&mut self, cache: *mut FClassNetCache, field: *mut c_void) -> Result<*mut FFieldNetCache, Self::Error>;
    unsafe fn cache_max_index(&mut self, cache: *mut FClassNetCache) -> Result<i32, Self::Error>;
    unsafe fn bunch_write_int(&mut self, bunch: *mut FOutBunch, value: i32, max_index: i32) -> Result<(), Self::Error>;
    unsafe fn bunch_write_byte(&mut self, bunch: *mut FOutBunch, value: u8) -> Result<(), Self::Error>;
    unsafe fn property_net_serialize(
        &mut self,
        property: *mut UProperty,
        bunch: *mut FOutBunch,
        package_map: *mut UPackageMap,
        data: *mut u8,
    ) -> Result<bool, Self::Error>;
    unsafe fn property_copy_single(&mut self, property: *mut UProperty, destination: *mut u8, source: *mut u8) -> Result<(), Self::Error>;
    unsafe fn actor_level_num_reps_add(&mut self, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn bunch_mark_pop(&mut self, bunch: *mut FOutBunch, overflowed: i32, num: i32) -> Result<(), Self::Error>;
    unsafe fn mark_num_bits(&mut self, bunch: *mut FOutBunch) -> Result<i32, Self::Error>;
    unsafe fn send_bunch(&mut self, channel: *mut UChannel, bunch: *mut FOutBunch, merge: i32) -> Result<i32, Self::Error>;
    unsafe fn array_remove_item_i32(&mut self, array: *mut TArrayRaw, index: i32) -> Result<(), Self::Error>;
    unsafe fn retirement_set_out(&mut self, channel: *mut UActorChannel, index: i32, packet: i32) -> Result<(), Self::Error>;
    unsafe fn retirement_set_reliable(&mut self, retirement: *mut c_void, reliable: i32) -> Result<(), Self::Error>;
    unsafe fn conn_add_sent_temporary(&mut self, connection: *mut UNetConnection, actor: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn driver_time_into(&mut self, driver: *mut UNetDriver, destination: *mut u8) -> Result<(), Self::Error>;
}

/// Applies the actor-channel assignment algorithm through a native or Core operation adapter.
///
/// # Safety
/// `channel`, `actor`, and all backing engine records must remain live for this call.
pub(crate) unsafe fn set_actor_with<O: ActorSetActorOps>(
    channel: *mut UActorChannel,
    actor: *mut c_void,
    ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        let base = channel.cast::<UChannel>();
        if (*base).closing != 0 {
            let expression = std::ffi::CStr::from_bytes_with_nul_unchecked(b"!Closing\0");
            ops.check_fail(expression, 648)?;
        }
        if !(*channel).actor.is_null() {
            let expression = std::ffi::CStr::from_bytes_with_nul_unchecked(b"Actor==NULL\0");
            ops.check_fail(expression, 649)?;
        }
        (*channel).actor = actor;
        (*channel).actor_class = ops.actor_class(actor)?.cast();
        let connection = (*base).connection.cast::<UNetConnection>();
        let package_map = (*connection).package_map.cast::<UPackageMap>();
        let cache = ops.class_net_cache(package_map, (*channel).actor_class.cast())?;
        ops.actor_channel_set(connection, actor, channel)?;
        ops.rep_eval_add_zeroed(channel, (*cache).rep_condition_count)?;
        let (_, temporary, _, _) = ops.actor_flags(actor)?;
        if temporary == 0 {
            let size = (*(*channel).actor_class.cast::<UClass>()).defaults.num;
            let recent = ops.recent_add(channel, size)?;
            ops.object_init_properties(recent, size, (*channel).actor_class.cast())?;
            let mut property = (*(*channel).actor_class.cast::<UStruct>()).config_link;
            while !property.is_null() {
                let metadata = ops.property_metadata(property.cast())?;
                let dest = recent.add(metadata.offset as usize);
                if metadata.flags & 0x0000_0008 != 0 {
                    ops.property_destroy_value(property.cast(), dest.cast())?;
                }
                if ops.property_is_bool(property.cast())? {
                    let mask = ops.property_bool_mask(property.cast())?;
                    *dest.cast::<u32>() &= !mask;
                } else {
                    ops.mem_zero(dest, metadata.element_size.wrapping_mul(metadata.array_dim))?;
                }
                property = (*property.cast::<UProperty>()).config_link_next;
            }
        }
        let reps = (*(*channel).actor_class.cast::<UClass>()).class_reps.num;
        ops.retirement_empty(channel, reps)?;
        let mut retirement_count = ops.retirement_num(channel)?;
        while retirement_count < reps {
            ops.retirement_add(channel)?;
            retirement_count += 1;
        }
        Ok(())
    }
}

/// Decodes and applies an actor-channel bunch through the supplied backend operations.
///
/// # Safety
/// `channel` and `bunch` plus all referenced engine objects must remain valid during processing.
pub(crate) unsafe fn actor_received_bunch_with<O: ActorReceiveOps>(
    channel: *mut UActorChannel,
    bunch: *mut FInBunch,
    ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        let base = channel.cast::<UChannel>();
        if (*base).closing != 0 {
            let expression = std::ffi::CStr::from_bytes_with_nul_unchecked(b"!Closing\0");
            ops.check_fail(expression, 704)?;
        }
        if (*base).broken != 0 { return Ok(()); }
        let mut just_spawned = false;
        if (*channel).actor.is_null() {
            if (*bunch).b_open == 0 {
                ops.invalid_new_channel_packet(bunch)?;
                (*base).broken = 1;
                return Ok(());
            }
            let object = ops.serialize_object(bunch)?;
            let mut actor = object;
            if !ops.object_is_actor(object)? {
                if !ops.object_is_class(object)? || !ops.class_is_actor_subclass(object)? {
                    ops.log_invalid_actor_class()?;
                    (*base).broken = 1;
                    return Ok(());
                }
                let location = ops.serialize_vector(bunch)?;
                actor = ops.spawn_actor((*channel).level.cast(), object, location)?;
                just_spawned = true;
                if actor.is_null() {
                    ops.log_actor_not_spawned()?;
                    (*base).broken = 1;
                    return Ok(());
                }
            }
            set_actor_with(channel, actor, ops)?;
        }
        let connection = (*base).connection.cast::<UNetConnection>();
        let driver = (*connection).driver.cast::<UNetDriver>();
        let class_cache = ops.class_net_cache((*connection).package_map.cast(), (*channel).actor_class.cast())?;
        (*(*channel).actor.cast::<AActor>()).set_b_net_owner(false);
        let mut top = (*channel).actor;
        while !(*top.cast::<AActor>()).owner.is_null() {
            top = (*top.cast::<AActor>()).owner;
        }
        let player = if ops.actor_is_player_pawn(top)? {
            ops.actor_player(top)?
        } else { ptr::null_mut() };
        let server_connection = (*driver).server_connection;
        if !server_connection.is_null() {
            if !player.is_null() && ops.player_is_viewport(player)? {
                (*(*channel).actor.cast::<AActor>()).set_b_net_owner(true);
            }
        } else if player == connection.cast() {
            (*(*channel).actor.cast::<AActor>()).set_b_net_owner(true);
        }
        let max_index = ops.cache_max_index(class_cache)?;
        let mut rep_index = ops.bunch_read_int(bunch, max_index)?;
        let mut bunch_error = ops.bunch_is_error(bunch)?;
        let mut field_cache = if bunch_error { ptr::null_mut() } else { ops.cache_from_index(class_cache, rep_index)? };
        while !field_cache.is_null() {
            ops.actor_pre_net_receive((*channel).actor)?;
            let mut field = (*field_cache).field;
            let mut kind = ops.field_kind(field)?;
            while !field_cache.is_null() && kind == 1 {
                let property = field.cast::<UProperty>();
                let metadata = ops.property_metadata(property)?;
                let element = if metadata.array_dim != 1 { ops.bunch_serialize_byte(bunch)? } else { 0 };
                let mut dest_actor = (*channel).actor.cast::<u8>();
                let mut dest_recent = if (*channel).recent.num != 0 { (*channel).recent.data.cast::<u8>() } else { ptr::null_mut() };
                if server_connection.is_null() {
                    let (role, remote_role) = ops.actor_roles((*channel).actor)?;
                    (*(*channel).actor.cast::<AActor>()).role = remote_role;
                    (*(*channel).actor.cast::<AActor>()).remote_role = role;
                    let owner_class = ops.property_owner_class(property)?;
                    let value = ops.frame_step_condition((*channel).actor, owner_class, (*property).rep_offset as i32)?;
                    (*(*channel).actor.cast::<AActor>()).role = role;
                    (*(*channel).actor.cast::<AActor>()).remote_role = remote_role;
                    if value == 0 || !(*(*channel).actor.cast::<AActor>()).b_net_owner() {
                        dest_actor = ptr::null_mut();
                        dest_recent = ptr::null_mut();
                    }
                }
                let retirement_index = metadata.index + i32::from(element);
                let retirement = ops.retirement_get(channel, retirement_index)?;
                let in_packet = ops.retirement_in_packet(retirement)?;
                if (*bunch).packet_id >= in_packet {
                    ops.retirement_set_in_packet(retirement, (*bunch).packet_id)?;
                } else {
                    dest_actor = ptr::null_mut();
                    dest_recent = ptr::null_mut();
                }
                ops.with_network_mem_scope(|ops| {
                    let data = if dest_actor.is_null() {
                        ops.mem_new_zeroed(metadata.element_size)?
                    } else {
                        dest_actor.add(metadata.offset as usize + element as usize * metadata.element_size as usize)
                    };
                    ops.property_net_serialize(property, bunch, (*connection).package_map.cast(), data)?;
                    if !dest_recent.is_null() {
                        ops.property_copy_single(
                            property,
                            dest_recent.add(metadata.offset as usize + element as usize * metadata.element_size as usize),
                            data,
                        )?;
                    }
                    Ok(())
                })?;
                rep_index = ops.bunch_read_int(bunch, max_index)?;
                bunch_error = ops.bunch_is_error(bunch)?;
                field_cache = if bunch_error { ptr::null_mut() } else { ops.cache_from_index(class_cache, rep_index)? };
                if !field_cache.is_null() {
                    field = (*field_cache).field;
                    kind = ops.field_kind(field)?;
                }
            }
            ops.actor_post_net_receive((*channel).actor)?;
            if just_spawned {
                ops.actor_post_net_begin_play((*channel).actor)?;
                just_spawned = false;
            }
            if !field_cache.is_null() && kind == 2 {
                let name = ops.field_name(field)?;
                let function = ops.function_find((*channel).actor, name)?;
                if function.is_null() {
                    let expression = std::ffi::CStr::from_bytes_with_nul_unchecked(b"Function\0");
                    ops.check_fail(expression, 856)?;
                    return Ok(());
                }
                let mut root = function;
                loop {
                    let parent = ops.function_super(root)?;
                    if parent.is_null() { break; }
                    root = parent;
                }
                let mut ignore = false;
                if server_connection.is_null() {
                    let (role, remote_role) = ops.actor_roles((*channel).actor)?;
                    (*(*channel).actor.cast::<AActor>()).role = remote_role;
                    (*(*channel).actor.cast::<AActor>()).remote_role = role;
                    let (owner_class, _) = ops.function_metadata(root)?;
                    let condition = ops.frame_step_condition((*channel).actor, owner_class, (*root).rep_offset as i32)?;
                    (*(*channel).actor.cast::<AActor>()).role = role;
                    (*(*channel).actor.cast::<AActor>()).remote_role = remote_role;
                    if condition == 0 || !(*(*channel).actor.cast::<AActor>()).b_net_owner() {
                        ignore = true;
                        ops.log_received_unwanted_function(name, (*channel).actor)?;
                    }
                }
                ops.with_network_mem_scope(|ops| {
                    let (_, params_size) = ops.function_metadata(function)?;
                    let params = ops.mem_new_zeroed(params_size)?;
                    let mut parameter = ops.function_param_first(function)?;
                    while !parameter.is_null() {
                        let flags = (*parameter).property_flags;
                        if flags & (hp2_core::class_metadata::METADATA_CPF_PARM
                            | hp2_core::class_metadata::METADATA_CPF_RETURN)
                            == hp2_core::class_metadata::METADATA_CPF_PARM
                            && ops.object_to_index((*connection).package_map.cast(), parameter.cast())?
                                != crate::prim::INDEX_NONE
                        {
                            let present = if ops.property_is_bool(parameter)? { true } else { ops.bunch_read_bit(bunch)? };
                            if present {
                                let metadata = ops.property_metadata(parameter)?;
                                ops.property_net_serialize(
                                    parameter,
                                    bunch,
                                    (*connection).package_map.cast(),
                                    params.add(metadata.offset as usize),
                                )?;
                            }
                        }
                        parameter = ops.function_param_next(parameter)?;
                    }
                    if !ignore {
                        ops.actor_set_client_demo_net_func((*channel).actor, true)?;
                        ops.actor_process_event((*channel).actor, function, params)?;
                    }
                    let mut property = (*function.cast::<UStruct>()).constructor_link;
                    while !property.is_null() {
                        let metadata = ops.property_metadata(property.cast())?;
                        if metadata.offset < params_size {
                            ops.property_destroy_value(property.cast(), params.add(metadata.offset as usize).cast())?;
                        }
                        property = (*property.cast::<UProperty>()).constructor_link_next;
                    }
                    Ok(())
                })?;
            } else if !field_cache.is_null() {
                return Ok(());
            }
            if !field_cache.is_null() {
                rep_index = ops.bunch_read_int(bunch, max_index)?;
                bunch_error = ops.bunch_is_error(bunch)?;
                field_cache = if bunch_error { ptr::null_mut() } else { ops.cache_from_index(class_cache, rep_index)? };
            }
        }
        if !driver.is_null() && server_connection == connection.cast() && (*connection).state == 2
            && (*(*channel).actor.cast::<AActor>()).b_net_owner()
            && ops.actor_is_player_pawn((*channel).actor)?
        {
            ops.actor_handle_client_player(connection, (*channel).actor)?;
        }
        Ok(())
    }
}


/// Shared `UActorChannel::ReplicateActor` protocol over native or Core leaves.
///
/// # Safety
/// `ch` and every reached engine record must remain live through the send scope.
pub(crate) unsafe fn actor_replicate_actor_with<O: ActorReplicateOps>(
    ch: *mut UActorChannel,
    ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        let base = ch.cast::<UChannel>();
        if (*ch).actor.is_null() {
            ops.check_fail(c"Actor", 941)?;
        }
        if (*base).closing != 0 {
            ops.check_fail(c"!Closing", 942)?;
        }
        ops.with_out_bunch(base, 0, |ops, bunch| {
            if ops.out_bunch_is_error(bunch)? {
                return Ok(());
            }
            let conn = (*base).connection.cast::<UNetConnection>();
            let driver = (*conn).driver;
            let driver_struct = driver.cast::<UNetDriver>();
            if (*base).open_packet_id != crate::prim::INDEX_NONE {
                (*(*ch).actor.cast::<AActor>()).set_b_net_initial(false);
                if (*ch).spawn_acked == 0 && (*base).open_acked != 0 {
                    (*ch).spawn_acked = 1;
                    let count = ops.array_num(ptr::addr_of!((*ch).retirement))?;
                    for i in (0..count).rev() {
                        let retirement = ops.retirement_get(ch, i)?;
                        let packet = ops.retirement_out_packet(retirement)?;
                        let reliable = ops.retirement_reliable(retirement)?;
                        if packet != crate::prim::INDEX_NONE && reliable == 0 {
                            ops.array_add_unique_i32(ptr::addr_of_mut!((*ch).dirty), i)?;
                        }
                    }
                }
            } else {
                let (_, temporary, _, _) = ops.actor_flags((*ch).actor)?;
                (*(*ch).actor.cast::<AActor>()).set_b_net_initial(true);
                (*bunch).b_close = temporary as u8;
                (*bunch).b_reliable = (temporary == 0) as u8;
            }

            let actor_class = ops.actor_class((*ch).actor)?;
            let class_cache = ops.class_net_cache((*conn).package_map.cast(), actor_class)?;
            if class_cache.is_null() {
                ops.check_fail(c"ClassCache", 967)?;
            }

            (*(*ch).actor.cast::<AActor>()).set_b_net_owner(false);
            let mut top = (*ch).actor;
            while !(*top.cast::<AActor>()).owner.is_null() {
                top = (*top.cast::<AActor>()).owner;
            }
            let player = if ops.actor_is_player_pawn(top)? {
                ops.actor_player(top)?
            } else {
                ptr::null_mut()
            };
            let recording_demo = ops.driver_recording_demo(driver_struct)?;
            let server_connection = (*driver_struct).server_connection;
            if recording_demo {
                let client_demo = ops.actor_net_mode_client((*ch).actor)?;
                (*(*ch).actor.cast::<AActor>()).set_b_net_owner(
                    if client_demo {
                        !player.is_null() && ops.player_is_viewport(player)?
                    } else {
                        true
                    },
                );
            } else if !server_connection.is_null() {
                (*(*ch).actor.cast::<AActor>()).set_b_net_owner(
                    !player.is_null() && ops.player_is_viewport(player)?,
                );
            } else {
                (*(*ch).actor.cast::<AActor>()).set_b_net_owner(player == conn.cast());
            }

            let actor_initial = (*(*ch).actor.cast::<AActor>()).b_net_initial();
            if actor_initial && (*base).opened_locally != 0 {
                let (_, _, is_static, no_delete) = ops.actor_flags((*ch).actor)?;
                if is_static != 0 || no_delete != 0 {
                    let mut serialized_actor = (*ch).actor;
                    ops.pm_serialize_object(
                        (*conn).package_map.cast(),
                        bunch,
                        ptr::addr_of_mut!(serialized_actor),
                    )?;
                } else {
                    let mut serialized_class = actor_class.cast();
                    ops.pm_serialize_object(
                        (*conn).package_map.cast(),
                        bunch,
                        ptr::addr_of_mut!(serialized_class),
                    )?;
                    let mut location = {
                        let loc = (*(*ch).actor.cast::<AActor>()).location;
                        [loc.x, loc.y, loc.z]
                    };
                    ops.out_bunch_serialize_vector(bunch, location.as_mut_ptr().cast())?;
                    if (*ch).recent.num != 0 {
                        (*(*ch).recent.data.cast::<AActor>()).location =
                            FVector { x: location[0], y: location[1], z: location[2] };
                    }
                }
            }

            let (_actual_role, actual_remote_role) = ops.actor_roles((*ch).actor)?;
            let mut remote_role = actual_remote_role as i32;
            const ROLE_AUTONOMOUS_PROXY: i32 = 3;
            const ROLE_SIMULATED_PROXY: i32 = 2;
            if remote_role == ROLE_AUTONOMOUS_PROXY {
                let instigator = (*(*ch).actor.cast::<AActor>()).instigator;
                let instigator_net_owner = if instigator.is_null() {
                    0
                } else {
                    i32::from((*instigator.cast::<AActor>()).b_net_owner())
                };
                let actor_net_owner = i32::from((*(*ch).actor.cast::<AActor>()).b_net_owner());
                if (!instigator.is_null() && instigator_net_owner == 0 && actor_net_owner == 0)
                    || recording_demo
                {
                    remote_role = ROLE_SIMULATED_PROXY;
                    (*(*ch).actor.cast::<AActor>()).remote_role = remote_role as u8;
                }
            }
            let is_pawn = (*(*ch).actor.cast::<AActor>()).b_is_pawn();
            (*(*ch).actor.cast::<AActor>())
                .set_b_simulated_pawn(is_pawn && remote_role == ROLE_SIMULATED_PROXY);

            ops.with_mem_scope(|ops| {
                if (*ch).rep_eval.num != 0 {
                    ops.mem_zero((*ch).rep_eval.data.cast(), (*ch).rep_eval.num)?;
                }
                let retirement_count = ops.array_num(ptr::addr_of!((*ch).retirement))?;
                let reps = ops.mem_new_int(retirement_count)?;
                let compare_bin = if (*ch).recent.num != 0 {
                    (*ch).recent.data.cast()
                } else {
                    (*actor_class).defaults.data.cast()
                };
                let rep_property_count = (*class_cache).rep_properties.num;
                let retirement_data = (*ch).retirement.data.cast::<u8>();
                let (mut last_rep, _rep_count) = ops.actor_optimized_rep_list(
                    (*ch).actor,
                    compare_bin,
                    retirement_data.cast(),
                    reps,
                    (*conn).package_map.cast(),
                    (*ch).num_reps,
                )?;
                (*ch).num_reps += 1;

                if ops.actor_should_script_replicate((*ch).actor)? {
                    for field_index in 0..rep_property_count {
                        let field_cache = ops.cache_rep_property_at(class_cache, field_index)?;
                        let condition_index = (*field_cache).condition_index;
                        let eval = *(*ch).rep_eval.data.cast::<u8>().add(condition_index as usize);
                        if eval == 2 {
                            continue;
                        }
                        let property = (*field_cache).field.cast::<UProperty>();
                        let is_object_property = ops.property_is_object(property)?;
                        let metadata = ops.property_metadata(property)?;
                        for element in 0..metadata.array_dim {
                            let property_offset = metadata.offset + element * metadata.element_size;
                            let mut source = (*ch).actor.cast::<u8>().add(property_offset as usize);
                            if is_object_property
                                && !ops.can_serialize_object((*conn).package_map.cast(), source.cast())?
                            {
                                source = ptr::null_mut();
                            }
                            if !ops.property_identical(
                                property,
                                compare_bin.add(property_offset as usize),
                                source,
                            )? {
                                let mut eval_value = eval;
                                if eval & 2 == 0 {
                                    let (rep_owner, rep_offset) = ops.property_replicated_metadata(property)?;
                                    let owner_class = ops.property_owner_class(rep_owner.cast())?;
                                    let value = ops.frame_step_condition((*ch).actor, owner_class, rep_offset)?;
                                    eval_value = (value | 2) as u8;
                                    *(*ch).rep_eval.data.cast::<u8>().add(condition_index as usize) = eval_value;
                                }
                                if eval_value & 1 != 0 {
                                    *last_rep = metadata.index + element;
                                    last_rep = last_rep.add(1);
                                }
                            }
                        }
                    }
                }

                if ops.out_bunch_is_error(bunch)? {
                    ops.check_fail(c"!Bunch.IsError()", 1073)?;
                }

                let dirty_count = ops.array_num(ptr::addr_of!((*ch).dirty))?;
                for dirty_index in (0..dirty_count).rev() {
                    let dirty = *(*ch).dirty.data.cast::<i32>().add(dirty_index as usize);
                    let mut cursor = reps;
                    while cursor < last_rep && *cursor != dirty {
                        cursor = cursor.add(1);
                    }
                    if cursor == last_rep {
                        *last_rep = dirty;
                        last_rep = last_rep.add(1);
                    }
                }

                let mut filled_up = 0;
                let mut cursor = reps;
                while cursor < last_rep {
                    let rep_index = *cursor;
                    let record = ops.class_rep_record(actor_class, rep_index)?;
                    let (property, element) = ops.rep_record_metadata(record)?;
                    let metadata = ops.property_metadata(property)?;
                    let property_offset = metadata.offset + element * metadata.element_size;
                    let name_kind = ops.property_name_kind(property)?;
                    let field_cache = if name_kind == 1 {
                        ops.cache_from_field(class_cache, (*driver_struct).remote_role_property)?
                    } else if name_kind == 2 {
                        ops.cache_from_field(class_cache, (*driver_struct).role_property)?
                    } else {
                        ops.cache_from_field(class_cache, property.cast())?
                    };
                    if field_cache.is_null() {
                        ops.check_fail(c"FieldCache", 1105)?;
                    }

                    let max_index = ops.cache_max_index(class_cache)?;
                    let writer = bunch.cast::<hp2_abi::engine::generated::FBitWriter>();
                    let bit_mark = ((*writer).ar_is_error, (*writer).num);
                    let field_index = (*field_cache).field_net_index;
                    ops.bunch_write_int(bunch, field_index, max_index)?;
                    if metadata.array_dim != 1 {
                        ops.bunch_write_byte(bunch, element as u8)?;
                    }
                    let source = (*ch).actor.cast::<u8>().add(property_offset as usize);
                    let mapped = ops.property_net_serialize(
                        property,
                        bunch,
                        (*conn).package_map.cast(),
                        source,
                    )?;
                    if !ops.out_bunch_is_error(bunch)? {
                        if (*ch).recent.num != 0 {
                            let recent = (*ch).recent.data.cast::<u8>().add(property_offset as usize);
                            if mapped {
                                ops.property_copy_single(property, recent, source)?;
                            } else {
                                ops.mem_zero(recent, metadata.element_size)?;
                            }
                        }
                        ops.actor_level_num_reps_add((*ch).actor)?;
                    } else {
                        ops.bunch_mark_pop(bunch, bit_mark.0, bit_mark.1)?;
                        last_rep = cursor;
                        filled_up = 1;
                        break;
                    }
                    cursor = cursor.add(1);
                }

                let num_bits = ops.mark_num_bits(bunch)?;
                if num_bits != 0 {
                    let packet_id = ops.send_bunch(base, bunch, 1)?;
                    let mut rep = reps;
                    while rep < last_rep {
                        ops.array_remove_item_i32(ptr::addr_of_mut!((*ch).dirty), *rep)?;
                        let retirement = ops.retirement_get(ch, *rep)?;
                        ops.retirement_set_out(ch, *rep, packet_id)?;
                        ops.retirement_set_reliable(retirement, (*bunch).b_reliable as i32)?;
                        rep = rep.add(1);
                    }
                    let (_, temporary, _, _) = ops.actor_flags((*ch).actor)?;
                    if temporary != 0 {
                        ops.conn_add_sent_temporary(conn, (*ch).actor)?;
                    }
                }
                if filled_up == 0 {
                    ops.driver_time_into(driver_struct, (*ch).last_update_time.as_mut_ptr())?;
                }
                Ok(())
            })?;
            (*(*ch).actor.cast::<AActor>()).set_b_net_owner(false);
            (*(*ch).actor.cast::<AActor>()).remote_role = actual_remote_role;
            Ok(())
        })
    }
}


struct NativeActorOps<'a> {
    services: &'a Hp2NetChanServices,
}

impl ActorSetActorOps for NativeActorOps<'_> {
    type Error = Thrown;
    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Thrown> {
        call!((self.services.chan_check_failed)(expression.as_ptr(), b"UnChan.cpp\0".as_ptr().cast(), line));
        Err(Thrown)
    }
    unsafe fn actor_class(&mut self, actor: *mut c_void) -> Result<*mut UClass, Thrown> {
        let mut class = ptr::null_mut();
        call!((self.services.chan_actor_get_class)(actor, &mut class));
        Ok(class.cast())
    }
    unsafe fn class_net_cache(&mut self, map: *mut UPackageMap, class: *mut UClass) -> Result<*mut FClassNetCache, Thrown> {
        let mut cache = ptr::null_mut();
        call!((self.services.chan_map_class_cache)(map.cast(), class.cast(), &mut cache));
        Ok(cache.cast())
    }
    unsafe fn actor_channel_set(&mut self, connection: *mut UNetConnection, actor: *mut c_void, channel: *mut UActorChannel) -> Result<(), Thrown> {
        call!((self.services.chan_actor_channels_set)(ptr::addr_of_mut!((*connection).actor_channels).cast(), actor, channel.cast()));
        Ok(())
    }
    unsafe fn rep_eval_add_zeroed(&mut self, channel: *mut UActorChannel, count: i32) -> Result<(), Thrown> {
        let mut start = 0;
        call!((self.services.chan_array_add_zeroed)(ptr::addr_of_mut!((*channel).rep_eval), count, &mut start));
        Ok(())
    }
    unsafe fn actor_flags(&mut self, actor: *mut c_void) -> Result<(i32, i32, i32, i32), Thrown> {
        let (mut valid, mut temporary, mut is_static, mut no_delete) = (0, 0, 0, 0);
        call!((self.services.chan_actor_flags)(actor, &mut valid, &mut temporary, &mut is_static, &mut no_delete));
        Ok((valid, temporary, is_static, no_delete))
    }
    unsafe fn recent_add(&mut self, channel: *mut UActorChannel, size: i32) -> Result<*mut u8, Thrown> {
        let mut start = 0;
        call!((self.services.chan_recent_add)(ptr::addr_of_mut!((*channel).recent), size, &mut start));
        Ok((*channel).recent.data.cast::<u8>().add(start as usize))
    }
    unsafe fn object_init_properties(&mut self, data: *mut u8, size: i32, class: *mut UClass) -> Result<(), Thrown> {
        call!((self.services.chan_object_init_properties)(data, size, class.cast()));
        Ok(())
    }
    unsafe fn property_metadata(&mut self, property: *mut UProperty) -> Result<ActorPropertyMetadata, Thrown> {
        let (mut offset, mut element_size, mut array_dim, mut index, mut flags) = (0, 0, 0, 0, 0);
        call!((self.services.chan_property_metadata)(property.cast(), &mut offset, &mut element_size, &mut array_dim, &mut index, &mut flags));
        Ok(ActorPropertyMetadata { offset, element_size, array_dim, index, flags })
    }
    unsafe fn property_bool_mask(&mut self, property: *mut UProperty) -> Result<u32, Thrown> {
        Ok((*property.cast::<UBoolProperty>()).bit_mask)
    }
    unsafe fn property_is_bool(&mut self, property: *mut UProperty) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_property_is_bool)(property.cast(), &mut value));
        Ok(value != 0)
    }
    unsafe fn property_destroy_value(&mut self, property: *mut UProperty, data: *mut c_void) -> Result<(), Thrown> {
        call!((self.services.chan_property_destroy_value)(property.cast(), data.cast()));
        Ok(())
    }
    unsafe fn mem_zero(&mut self, data: *mut u8, size: i32) -> Result<(), Thrown> {
        call!((self.services.chan_mem_zero)(data, size));
        Ok(())
    }
    unsafe fn retirement_empty(&mut self, channel: *mut UActorChannel, count: i32) -> Result<(), Thrown> {
        call!((self.services.chan_retirement_empty)(ptr::addr_of_mut!((*channel).retirement), count));
        Ok(())
    }
    unsafe fn retirement_num(&mut self, channel: *mut UActorChannel) -> Result<i32, Thrown> {
        let mut count = 0;
        call!((self.services.chan_array_num)(ptr::addr_of!((*channel).retirement), &mut count));
        Ok(count)
    }
    unsafe fn retirement_add(&mut self, channel: *mut UActorChannel) -> Result<(), Thrown> {
        let mut index = 0;
        call!((self.services.chan_retirement_add)(ptr::addr_of_mut!((*channel).retirement), &mut index));
        Ok(())
    }
}

impl ActorReceiveOps for NativeActorOps<'_> {
    unsafe fn invalid_new_channel_packet(&mut self, bunch: *mut FInBunch) -> Result<(), Thrown> {
        call!((self.services.chan_log_invalid_new_channel_packet)((*bunch).b_open as i32, (*bunch).b_close as i32, (*bunch).b_reliable as i32));
        Ok(())
    }
    unsafe fn serialize_object(&mut self, bunch: *mut FInBunch) -> Result<*mut c_void, Thrown> {
        let mut object = ptr::null_mut();
        call!((self.services.chan_bunch_serialize_object)(bunch, &mut object));
        Ok(object)
    }
    unsafe fn object_is_actor(&mut self, object: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_object_is_actor)(object, &mut value));
        Ok(value != 0)
    }
    unsafe fn object_is_class(&mut self, object: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_object_is_class)(object, &mut value));
        Ok(value != 0)
    }
    unsafe fn class_is_actor_subclass(&mut self, class: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_class_is_actor_subclass)(class, &mut value));
        Ok(value != 0)
    }
    unsafe fn log_invalid_actor_class(&mut self) -> Result<(), Thrown> {
        call!((self.services.chan_log_invalid_actor_class)());
        Ok(())
    }
    unsafe fn serialize_vector(&mut self, bunch: *mut FInBunch) -> Result<FVector, Thrown> {
        let mut value = FVector { x: 0.0, y: 0.0, z: 0.0 };
        call!((self.services.chan_bunch_serialize_vector)(bunch, (&mut value as *mut FVector).cast()));
        Ok(value)
    }
    unsafe fn spawn_actor(&mut self, level: *mut c_void, class: *mut c_void, location: FVector) -> Result<*mut c_void, Thrown> {
        crate::level_spawn_actor::virtual_spawn_actor(
            level, class, FName { index: 0 }, ptr::null_mut(), ptr::null_mut(), location,
            FRotator { pitch: 0, yaw: 0, roll: 0 }, ptr::null_mut(), 1, 1,
        ).map(|actor| actor as *mut c_void).map_err(|_| Thrown)
    }
    unsafe fn log_actor_not_spawned(&mut self) -> Result<(), Thrown> {
        call!((self.services.chan_log_actor_not_spawned)());
        Ok(())
    }
    unsafe fn actor_pre_net_receive(&mut self, actor: *mut c_void) -> Result<(), Thrown> {
        call!((self.services.chan_actor_pre_net_receive)(actor));
        Ok(())
    }
    unsafe fn actor_post_net_receive(&mut self, actor: *mut c_void) -> Result<(), Thrown> {
        call!((self.services.chan_actor_post_net_receive)(actor));
        Ok(())
    }
    unsafe fn actor_post_net_begin_play(&mut self, actor: *mut c_void) -> Result<(), Thrown> {
        call!((self.services.chan_actor_post_net_begin_play)(actor));
        Ok(())
    }
    unsafe fn actor_is_player_pawn(&mut self, actor: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_actor_is_player_pawn)(actor, &mut value));
        Ok(value != 0)
    }
    unsafe fn actor_player(&mut self, actor: *mut c_void) -> Result<*mut c_void, Thrown> {
        let mut player = ptr::null_mut();
        call!((self.services.chan_actor_player)(actor, &mut player));
        Ok(player)
    }
    unsafe fn player_is_viewport(&mut self, player: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_player_is_viewport)(player, &mut value));
        Ok(value != 0)
    }
    unsafe fn cache_max_index(&mut self, cache: *mut FClassNetCache) -> Result<i32, Thrown> {
        let mut value = 0;
        call!((self.services.chan_cache_max_index)(cache.cast(), &mut value));
        Ok(value)
    }
    unsafe fn bunch_read_int(&mut self, bunch: *mut FInBunch, max_index: i32) -> Result<i32, Thrown> {
        let mut value = 0;
        call!((self.services.chan_bunch_read_int)(bunch, max_index, &mut value));
        Ok(value)
    }
    unsafe fn bunch_is_error(&mut self, bunch: *mut FInBunch) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_bunch_is_error)(bunch, &mut value));
        Ok(value != 0)
    }
    unsafe fn cache_from_index(&mut self, cache: *mut FClassNetCache, index: i32) -> Result<*mut FFieldNetCache, Thrown> {
        let mut field = ptr::null_mut();
        call!((self.services.chan_cache_from_index)(cache.cast(), index, &mut field));
        Ok(field.cast())
    }
    unsafe fn field_kind(&mut self, field: *mut c_void) -> Result<i32, Thrown> {
        let mut value = 0;
        call!((self.services.chan_field_kind)(field, &mut value));
        Ok(value)
    }
    unsafe fn property_owner_class(&mut self, property: *mut UProperty) -> Result<*mut UClass, Thrown> {
        let mut class = ptr::null_mut();
        call!((self.services.chan_property_owner_class)(property.cast(), &mut class));
        Ok(class.cast())
    }
    unsafe fn actor_roles(&mut self, actor: *mut c_void) -> Result<(u8, u8), Thrown> {
        let (mut role, mut remote) = (0, 0);
        call!((self.services.chan_actor_get_roles)(actor, &mut role, &mut remote));
        Ok((role as u8, remote as u8))
    }
    unsafe fn frame_step_condition(&mut self, actor: *mut c_void, owner_class: *mut UClass, offset: i32) -> Result<u32, Thrown> {
        let mut value = 0;
        call!((self.services.chan_frame_step_condition)(actor, owner_class.cast(), offset, ptr::null_mut(), &mut value));
        Ok(value)
    }
    unsafe fn retirement_get(&mut self, channel: *mut UActorChannel, index: i32) -> Result<*mut c_void, Thrown> {
        let mut value = ptr::null_mut();
        call!((self.services.chan_retirement_get)(ptr::addr_of_mut!((*channel).retirement).cast(), index, &mut value));
        Ok(value)
    }
    unsafe fn retirement_in_packet(&mut self, retirement: *mut c_void) -> Result<i32, Thrown> {
        let mut value = 0;
        call!((self.services.chan_retirement_in_packet)(retirement, &mut value));
        Ok(value)
    }
    unsafe fn retirement_set_in_packet(&mut self, retirement: *mut c_void, packet: i32) -> Result<(), Thrown> {
        call!((self.services.chan_retirement_set_in_packet)(retirement, packet));
        Ok(())
    }
    unsafe fn with_network_mem_scope<T>(&mut self, body: impl FnOnce(&mut Self) -> Result<T, Thrown>) -> Result<T, Thrown> {
        let scope = NetworkMemScope::new((self.services.chan_mem_stack)());
        let result = body(self);
        drop(scope);
        result
    }
    unsafe fn mem_new_zeroed(&mut self, size: i32) -> Result<*mut u8, Thrown> {
        let mut value = ptr::null_mut();
        call!((self.services.chan_mem_new_zeroed)(size, &mut value));
        Ok(value)
    }
    unsafe fn property_net_serialize(&mut self, property: *mut UProperty, bunch: *mut FInBunch, map: *mut UPackageMap, data: *mut u8) -> Result<(), Thrown> {
        let mut mapped = 0;
        call!((self.services.chan_property_net_serialize)(property.cast(), bunch.cast(), map.cast(), data, &mut mapped));
        Ok(())
    }
    unsafe fn property_copy_single(&mut self, property: *mut UProperty, destination: *mut u8, source: *mut u8) -> Result<(), Thrown> {
        call!((self.services.chan_property_copy_single)(property.cast(), destination, source));
        Ok(())
    }
    unsafe fn bunch_serialize_byte(&mut self, bunch: *mut FInBunch) -> Result<u8, Thrown> {
        let mut value = 0;
        call!((self.services.chan_bunch_serialize_byte)(bunch, &mut value));
        Ok(value)
    }
    unsafe fn field_name(&mut self, field: *mut c_void) -> Result<i32, Thrown> {
        let mut value = 0;
        call!((self.services.chan_field_name)(field, (&mut value as *mut i32).cast()));
        Ok(value)
    }
    unsafe fn function_find(&mut self, actor: *mut c_void, name: i32) -> Result<*mut UFunction, Thrown> {
        let mut name = name;
        let mut function = ptr::null_mut();
        call!((self.services.chan_function_find)(actor, (&mut name as *mut i32).cast(), &mut function));
        Ok(function.cast())
    }
    unsafe fn function_super(&mut self, function: *mut UFunction) -> Result<*mut UFunction, Thrown> {
        let mut parent = ptr::null_mut();
        call!((self.services.chan_function_super)(function.cast(), &mut parent));
        Ok(parent.cast())
    }
    unsafe fn function_metadata(&mut self, function: *mut UFunction) -> Result<(*mut UClass, i32), Thrown> {
        let (mut class, mut size) = (ptr::null_mut(), 0);
        let (mut metadata_a, mut metadata_b) = (ptr::null_mut(), ptr::null_mut());
        call!((self.services.chan_function_metadata)(function.cast(), &mut class, &mut size, &mut metadata_a, &mut metadata_b));
        Ok((class.cast(), size))
    }
    unsafe fn log_received_unwanted_function(&mut self, name: i32, actor: *mut c_void) -> Result<(), Thrown> {
        let mut name = name;
        call!((self.services.chan_log_received_unwanted_function)((&mut name as *mut i32).cast(), actor));
        Ok(())
    }
    unsafe fn function_param_first(&mut self, function: *mut UFunction) -> Result<*mut UProperty, Thrown> {
        let mut parameter = ptr::null_mut();
        call!((self.services.chan_function_param_first)(function.cast(), &mut parameter));
        Ok(parameter.cast())
    }
    unsafe fn function_param_next(&mut self, parameter: *mut UProperty) -> Result<*mut UProperty, Thrown> {
        let mut next = ptr::null_mut();
        call!((self.services.chan_function_param_next)(parameter.cast(), &mut next));
        Ok(next.cast())
    }
    unsafe fn object_to_index(&mut self, map: *mut UPackageMap, object: *mut c_void) -> Result<i32, Thrown> {
        let mut index = crate::prim::INDEX_NONE;
        call!((self.services.chan_object_to_index)(map.cast(), object, &mut index));
        Ok(index)
    }
    unsafe fn bunch_read_bit(&mut self, bunch: *mut FInBunch) -> Result<bool, Thrown> {
        let mut value = 0;
        call!((self.services.chan_bunch_read_bit)(bunch, &mut value));
        Ok(value != 0)
    }
    unsafe fn actor_set_client_demo_net_func(&mut self, actor: *mut c_void, value: bool) -> Result<(), Thrown> {
        (*actor.cast::<AActor>()).set_b_client_demo_net_func(value);
        Ok(())
    }
    unsafe fn actor_process_event(&mut self, actor: *mut c_void, function: *mut UFunction, parameters: *mut u8) -> Result<(), Thrown> {
        call!((self.services.chan_actor_process_event)(actor, function.cast(), parameters));
        Ok(())
    }
    unsafe fn actor_handle_client_player(&mut self, connection: *mut UNetConnection, actor: *mut c_void) -> Result<(), Thrown> {
        call!((self.services.chan_actor_handle_client_player)(connection.cast(), actor));
        Ok(())
    }
}


impl ActorReplicateOps for NativeActorOps<'_> {
    unsafe fn with_out_bunch<T>(
        &mut self,
        channel: *mut UChannel,
        close: i32,
        body: impl FnOnce(&mut Self, *mut FOutBunch) -> Result<T, Thrown>,
    ) -> Result<T, Thrown> {
        let services = self.services;
        unsafe { net_bunch::with_out(&services.bunches, channel, close, |bunch| body(self, bunch)) }
    }

    unsafe fn with_mem_scope<T>(
        &mut self,
        body: impl FnOnce(&mut Self) -> Result<T, Thrown>,
    ) -> Result<T, Thrown> {
        unsafe {
            let scope = NetworkMemScope::new((self.services.chan_mem_stack)());
            let result = body(self);
            drop(scope);
            result
        }
    }

    unsafe fn out_bunch_is_error(&mut self, bunch: *mut FOutBunch) -> Result<bool, Thrown> {
        let mut error = 0;
        unsafe { call!((self.services.chan_out_bunch_is_error)(bunch, &mut error)); }
        Ok(error != 0)
    }

    unsafe fn array_num(&mut self, array: *const TArrayRaw) -> Result<i32, Thrown> {
        let mut count = 0;
        unsafe { call!((self.services.chan_array_num)(array, &mut count)); }
        Ok(count)
    }

    unsafe fn retirement_get(&mut self, channel: *mut UActorChannel, index: i32) -> Result<*mut c_void, Thrown> {
        let mut retirement = ptr::null_mut();
        unsafe { call!((self.services.chan_retirement_get)(ptr::addr_of_mut!((*channel).retirement).cast(), index, &mut retirement)); }
        Ok(retirement)
    }

    unsafe fn retirement_out_packet(&mut self, retirement: *mut c_void) -> Result<i32, Thrown> {
        let mut packet = 0;
        unsafe { call!((self.services.chan_retirement_out_packet)(retirement, &mut packet)); }
        Ok(packet)
    }

    unsafe fn retirement_reliable(&mut self, retirement: *mut c_void) -> Result<i32, Thrown> {
        let mut reliable = 0;
        unsafe { call!((self.services.chan_retirement_reliable)(retirement, &mut reliable)); }
        Ok(reliable)
    }

    unsafe fn array_add_unique_i32(&mut self, array: *mut TArrayRaw, value: i32) -> Result<(), Thrown> {
        let mut inserted = 0;
        unsafe { call!((self.services.chan_array_add_unique_i32)(array, value, &mut inserted)); }
        Ok(())
    }

    unsafe fn actor_is_player_pawn(&mut self, actor: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_actor_is_player_pawn)(actor, &mut value)); }
        Ok(value != 0)
    }

    unsafe fn actor_player(&mut self, actor: *mut c_void) -> Result<*mut c_void, Thrown> {
        let mut player = ptr::null_mut();
        unsafe { call!((self.services.chan_actor_player)(actor, &mut player)); }
        Ok(player)
    }

    unsafe fn player_is_viewport(&mut self, player: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_player_is_viewport)(player, &mut value)); }
        Ok(value != 0)
    }

    unsafe fn driver_recording_demo(&mut self, driver: *mut UNetDriver) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_driver_recording_demo)(driver.cast(), &mut value)); }
        Ok(value != 0)
    }

    unsafe fn actor_net_mode_client(&mut self, actor: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_actor_net_mode_client)(actor, &mut value)); }
        Ok(value != 0)
    }

    unsafe fn pm_serialize_object(
        &mut self,
        map: *mut UPackageMap,
        bunch: *mut FOutBunch,
        object: *mut *mut c_void,
    ) -> Result<bool, Thrown> {
        unsafe { call!((self.services.chan_pm_serialize_object)(map.cast(), bunch.cast(), object)); }
        Ok(true)
    }

    unsafe fn out_bunch_serialize_vector(&mut self, bunch: *mut FOutBunch, vector: *mut FVector) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_out_bunch_serialize_vector)(bunch, vector.cast())); }
        Ok(())
    }

    unsafe fn actor_roles(&mut self, actor: *mut c_void) -> Result<(u8, u8), Thrown> {
        let (mut role, mut remote) = (0, 0);
        unsafe { call!((self.services.chan_actor_get_roles)(actor, &mut role, &mut remote)); }
        Ok((role as u8, remote as u8))
    }

    unsafe fn mem_new_int(&mut self, count: i32) -> Result<*mut i32, Thrown> {
        let mut value = ptr::null_mut();
        unsafe { call!((self.services.chan_mem_new_int)(count, &mut value)); }
        Ok(value)
    }

    unsafe fn actor_optimized_rep_list(
        &mut self,
        actor: *mut c_void,
        compare: *mut u8,
        retirement: *mut c_void,
        reps: *mut i32,
        map: *mut UPackageMap,
        num_reps: i32,
    ) -> Result<(*mut i32, i32), Thrown> {
        let mut last = ptr::null_mut();
        let mut count = 0;
        unsafe {
            call!((self.services.chan_actor_optimized_rep_list)(
                actor, compare, retirement, reps, map.cast(), num_reps, &mut last, &mut count,
            ));
        }
        Ok((last, count))
    }

    unsafe fn actor_should_script_replicate(&mut self, actor: *mut c_void) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_actor_should_script_replicate)(actor, &mut value)); }
        Ok(value != 0)
    }

    unsafe fn cache_rep_property_at(&mut self, cache: *mut FClassNetCache, index: i32) -> Result<*mut FFieldNetCache, Thrown> {
        let mut field = ptr::null_mut();
        unsafe { call!((self.services.chan_cache_rep_property_at)(cache.cast(), index, &mut field)); }
        Ok(field.cast())
    }

    unsafe fn property_is_object(&mut self, property: *mut UProperty) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_property_is_object)(property.cast(), &mut value)); }
        Ok(value != 0)
    }

    unsafe fn can_serialize_object(&mut self, map: *mut UPackageMap, object_value: *mut c_void) -> Result<bool, Thrown> {
        // `ChanCanSerializeObject` takes the object, while the channel body
        // holds the property slot.
        let object = unsafe { object_value.cast::<*mut c_void>().read_unaligned() };
        let mut value = 0;
        unsafe { call!((self.services.chan_can_serialize_object)(map.cast(), object, &mut value)); }
        Ok(value != 0)
    }

    unsafe fn property_identical(&mut self, property: *mut UProperty, a: *mut u8, b: *mut u8) -> Result<bool, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_property_identical)(property.cast(), a, b, &mut value)); }
        Ok(value != 0)
    }

    unsafe fn property_replicated_metadata(&mut self, property: *mut UProperty) -> Result<(*mut c_void, i32), Thrown> {
        let mut owner = ptr::null_mut();
        let mut offset = 0;
        unsafe { call!((self.services.chan_property_replicated_metadata)(property.cast(), &mut owner, &mut offset)); }
        Ok((owner, offset))
    }

    unsafe fn property_owner_class(&mut self, property: *mut UProperty) -> Result<*mut UClass, Thrown> {
        let mut class = ptr::null_mut();
        unsafe { call!((self.services.chan_property_owner_class)(property.cast(), &mut class)); }
        Ok(class.cast())
    }

    unsafe fn frame_step_condition(&mut self, actor: *mut c_void, owner_class: *mut UClass, offset: i32) -> Result<u32, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_frame_step_condition)(actor, owner_class.cast(), offset, ptr::null_mut(), &mut value)); }
        Ok(value)
    }

    unsafe fn class_rep_record(&mut self, class: *mut UClass, index: i32) -> Result<*mut c_void, Thrown> {
        let mut record = ptr::null_mut();
        unsafe { call!((self.services.chan_class_rep_record)(class.cast(), index, &mut record)); }
        Ok(record)
    }

    unsafe fn rep_record_metadata(&mut self, record: *mut c_void) -> Result<(*mut UProperty, i32), Thrown> {
        let mut property = ptr::null_mut();
        let mut index = 0;
        unsafe { call!((self.services.chan_rep_record_metadata)(record, &mut property, &mut index)); }
        Ok((property.cast(), index))
    }

    unsafe fn property_name_kind(&mut self, property: *mut UProperty) -> Result<i32, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_property_name_kind)(property.cast(), &mut value)); }
        Ok(value)
    }

    unsafe fn cache_from_field(&mut self, cache: *mut FClassNetCache, field: *mut c_void) -> Result<*mut FFieldNetCache, Thrown> {
        let mut result = ptr::null_mut();
        unsafe { call!((self.services.chan_cache_from_field)(cache.cast(), field, &mut result)); }
        Ok(result.cast())
    }

    unsafe fn cache_max_index(&mut self, cache: *mut FClassNetCache) -> Result<i32, Thrown> {
        let mut value = 0;
        unsafe { call!((self.services.chan_cache_max_index)(cache.cast(), &mut value)); }
        Ok(value)
    }

    unsafe fn bunch_write_int(&mut self, bunch: *mut FOutBunch, value: i32, max_index: i32) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_bunch_write_int)(bunch, value, max_index)); }
        Ok(())
    }

    unsafe fn bunch_write_byte(&mut self, bunch: *mut FOutBunch, value: u8) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_bunch_write_byte)(bunch, value)); }
        Ok(())
    }

    unsafe fn property_net_serialize(
        &mut self,
        property: *mut UProperty,
        bunch: *mut FOutBunch,
        package_map: *mut UPackageMap,
        data: *mut u8,
    ) -> Result<bool, Thrown> {
        let mut mapped = 0;
        unsafe {
            call!((self.services.chan_property_net_serialize)(
                property.cast(), bunch.cast(), package_map.cast(), data, &mut mapped,
            ));
        }
        Ok(mapped != 0)
    }

    unsafe fn property_copy_single(&mut self, property: *mut UProperty, destination: *mut u8, source: *mut u8) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_property_copy_single)(property.cast(), destination, source)); }
        Ok(())
    }

    unsafe fn actor_level_num_reps_add(&mut self, actor: *mut c_void) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_actor_level_num_reps_add)(actor)); }
        Ok(())
    }

    unsafe fn bunch_mark_pop(&mut self, bunch: *mut FOutBunch, overflowed: i32, num: i32) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_bunch_mark_pop)(bunch, overflowed, num)); }
        Ok(())
    }

    unsafe fn mark_num_bits(&mut self, bunch: *mut FOutBunch) -> Result<i32, Thrown> {
        let mut bits = 0;
        unsafe { call!((self.services.chan_mark_num_bits)(bunch.cast(), &mut bits)); }
        Ok(bits)
    }

    unsafe fn send_bunch(&mut self, channel: *mut UChannel, bunch: *mut FOutBunch, merge: i32) -> Result<i32, Thrown> {
        unsafe { channel_send_bunch(self.services, channel, bunch, merge) }
    }

    unsafe fn array_remove_item_i32(&mut self, array: *mut TArrayRaw, index: i32) -> Result<(), Thrown> {
        let mut removed = 0;
        unsafe { call!((self.services.chan_array_remove_item_i32)(array, index, &mut removed)); }
        Ok(())
    }

    unsafe fn retirement_set_out(&mut self, channel: *mut UActorChannel, index: i32, packet: i32) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_retirement_set_out)(ptr::addr_of_mut!((*channel).retirement).cast(), index, packet)); }
        Ok(())
    }

    unsafe fn retirement_set_reliable(&mut self, retirement: *mut c_void, reliable: i32) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_retirement_set_reliable)(retirement, reliable)); }
        Ok(())
    }

    unsafe fn conn_add_sent_temporary(&mut self, connection: *mut UNetConnection, actor: *mut c_void) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_conn_add_sent_temporary)(connection.cast(), actor)); }
        Ok(())
    }

    unsafe fn driver_time_into(&mut self, driver: *mut UNetDriver, destination: *mut u8) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_driver_time_into)(driver.cast(), destination)); }
        Ok(())
    }
}

pub unsafe fn actor_set_actor(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
    actor: *mut c_void,
) -> Result<(), Thrown> {
    unsafe {
        let mut ops = NativeActorOps { services: s };
        set_actor_with(ch, actor, &mut ops)
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_received_bunch(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
    bunch: *mut FInBunch,
) -> Result<(), Thrown> {
    unsafe {
        let mut ops = NativeActorOps { services: s };
        actor_received_bunch_with(ch, bunch, &mut ops)
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_replicate_actor(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
) -> Result<(), Thrown> {
    unsafe {
        let mut ops = NativeActorOps { services: s };
        actor_replicate_actor_with(ch, &mut ops)
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `Hp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn actor_describe(
    s: &Hp2NetChanServices,
    ch: *mut UActorChannel,
    out: *mut c_void,
) -> Result<(), Thrown> {
    unsafe { describe_actor_with(&mut NativeDescriptionOps { services: s, output: out }, ch) }
}
/// Borrowed package URL text; the package-map entry owns the FString storage.
#[derive(Clone, Copy)]
pub(crate) struct FileReceiveUrl {
    header: *mut c_void,
    data: *const u32,
    units: usize,
}

impl FileReceiveUrl {
    /// # Safety
    /// `header` is a live FString header that remains stable through one receive operation.
    pub(crate) unsafe fn from_header(header: *mut c_void) -> Option<Self> {
        if header.is_null() { return None; }
        unsafe {
            let array = header.cast::<TArrayRaw>();
            let num = (*array).num;
            let max = (*array).max;
            if num < 0 || max < num || (num > 0 && (*array).data.is_null()) {
                return None;
            }
            Some(Self {
                header,
                data: (*array).data.cast(),
                units: num as usize,
            })
        }
    }
    pub(crate) fn header(self) -> *mut c_void { self.header }
    /// # Safety
    /// The package-map FString backing this view must remain live and unchanged.
    pub(crate) unsafe fn as_slice<'a>(self) -> &'a [u32] {
        if self.units == 0 { &[] } else { unsafe { std::slice::from_raw_parts(self.data, self.units) } }
    }
    /// # Safety
    /// The package-map FString backing this view must remain live and readable.
    pub(crate) unsafe fn is_empty(self) -> bool {
        let units = unsafe { self.as_slice() };
        units.first().copied().unwrap_or(0) == 0
    }
}

pub(crate) struct FilePackageInfo {
    pub(crate) url: FileReceiveUrl,
    pub(crate) guid: [u32; 4],
}

/// Operations adapting native and Core file-channel receive services.
pub(crate) trait FileReceiveOps {
    type Error;
    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn read_bunch_skip(&mut self, bunch: *mut FInBunch) -> Result<(bool, bool), Self::Error>;
    unsafe fn read_bunch_guid(&mut self, bunch: *mut FInBunch) -> Result<([u32; 4], bool), Self::Error>;
    unsafe fn bunch_data(&mut self, bunch: *mut FInBunch) -> Result<(*const u8, i32), Self::Error>;
    unsafe fn receive_download_data(&mut self, download: *mut UDownload, data: *const u8, bytes: i32) -> Result<(), Self::Error>;
    unsafe fn log_file_invalid(&mut self) -> Result<(), Self::Error>;
    unsafe fn send_close_reply(&mut self, channel: *mut UChannel) -> Result<(), Self::Error>;
    unsafe fn package_count(&mut self, connection: *mut UNetConnection) -> Result<i32, Self::Error>;
    unsafe fn package_info(&mut self, connection: *mut UNetConnection, index: i32) -> Result<FilePackageInfo, Self::Error>;
    unsafe fn package_has_linker(&mut self, connection: *mut UNetConnection, index: i32) -> Result<bool, Self::Error>;
    unsafe fn remove_package(&mut self, connection: *mut UNetConnection, index: i32) -> Result<(), Self::Error>;
    unsafe fn set_source_filename(&mut self, channel: *mut UFileChannel, url: FileReceiveUrl, index: i32) -> Result<(), Self::Error>;
    unsafe fn notify_sending_file(&mut self, connection: *mut UNetConnection, guid: &[u32; 4]) -> Result<bool, Self::Error>;
    unsafe fn file_size(&mut self, url: FileReceiveUrl) -> Result<i32, Self::Error>;
    unsafe fn create_file_reader(&mut self, channel: *mut UFileChannel) -> Result<(), Self::Error>;
    unsafe fn log_file_skip(&mut self, channel: *mut UFileChannel) -> Result<(), Self::Error>;
    unsafe fn log_file_send(&mut self, channel: *mut UFileChannel) -> Result<(), Self::Error>;
    unsafe fn download_allowed(&mut self, connection: *mut UNetConnection) -> Result<bool, Self::Error>;
    unsafe fn sending_file(&mut self, channel: *mut UFileChannel) -> bool;
}

/// Shared `UFileChannel::ReceivedBunch` protocol and sequencing.
///
/// # Safety
/// `channel` and `bunch` must be live and the channel must retain its connection state.
pub(crate) unsafe fn file_received_bunch_with<O: FileReceiveOps>(
    ops: &mut O,
    channel: *mut UFileChannel,
    bunch: *mut FInBunch,
) -> Result<(), O::Error> {
    unsafe {
        let base = channel.cast::<UChannel>();
        if (*base).closing != 0 {
            ops.check_fail(c"!Closing", 1014)?;
        }
        let connection = (*base).connection.cast::<UNetConnection>();
        let driver = (*connection).driver.cast::<UNetDriver>();
        if (*base).opened_locally != 0 {
            let (data, bytes) = ops.bunch_data(bunch)?;
            ops.receive_download_data((*channel).download.cast(), data, bytes)?;
            return Ok(());
        }
        if !ops.download_allowed(connection)? {
            ops.log_file_invalid()?;
            return ops.send_close_reply(base);
        }
        if ops.sending_file(channel) {
            let (archive_error, is_skip) = ops.read_bunch_skip(bunch)?;
            if !archive_error && is_skip {
                ops.log_file_skip(channel)?;
                ops.remove_package(connection, (*channel.cast::<UFileChannel>()).package_index)?;
                return Ok(());
            }
        } else {
            let (guid, archive_error) = ops.read_bunch_guid(bunch)?;
            if !archive_error {
                for index in 0..ops.package_count(connection)? {
                    let info = ops.package_info(connection, index)?;
                    if info.guid != guid || info.url.is_empty() {
                        continue;
                    }
                    let max_size = (*driver).max_download_size;
                    if max_size > 0 && ops.file_size(info.url)? > max_size {
                        break;
                    }
                    ops.set_source_filename(channel, info.url, index)?;
                    if !ops.notify_sending_file(connection, &guid)? {
                        continue;
                    }
                    if !ops.package_has_linker(connection, index)? {
                        ops.check_fail(c"Info.Linker", 1078)?;
                    }
                    ops.create_file_reader(channel)?;
                    if ops.sending_file(channel) {
                        ops.log_file_send(channel)?;
                        (*channel.cast::<UFileChannel>()).package_index = index;
                        return Ok(());
                    }
                }
            }
        }
        ops.log_file_invalid()?;
        ops.send_close_reply(base)?;
    }
    Ok(())
}

struct NativeFileReceiveOps<'a>(&'a Hp2NetChanServices);


impl FileReceiveOps for NativeFileReceiveOps<'_> {
    type Error = Thrown;

    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_check_failed)(expression.as_ptr(), c"UnChan.cpp".as_ptr(), line)); }
        Ok(())
    }
    unsafe fn read_bunch_skip(&mut self, bunch: *mut FInBunch) -> Result<(bool, bool), Thrown> {
        static SKIP: [u32; 5] = [83, 75, 73, 80, 0];
        unsafe {
            net_bunch::with_string(|text| {
                call!((self.0.chan_in_bunch_serialize_string)(bunch, text));
                let mut error = 0;
                call!((self.0.chan_in_bunch_is_error)(bunch, &mut error));
                let mut is_skip = 0;
                if error == 0 {
                    call!((self.0.chan_string_equals_tchar)(text, SKIP.as_ptr(), &mut is_skip));
                }
                Ok((error != 0, is_skip != 0))
            })
        }
    }
    unsafe fn read_bunch_guid(&mut self, bunch: *mut FInBunch) -> Result<([u32; 4], bool), Thrown> {
        let mut guid = [0; 4];
        let mut error = 0;
        unsafe {
            call!((self.0.chan_in_bunch_serialize_guid)(bunch, guid.as_mut_ptr().cast()));
            call!((self.0.chan_in_bunch_is_error)(bunch, &mut error));
        }
        Ok((guid, error != 0))
    }
    unsafe fn bunch_data(&mut self, bunch: *mut FInBunch) -> Result<(*const u8, i32), Thrown> {
        let mut bytes = 0;
        let mut data = std::ptr::null();
        unsafe {
            call!((self.0.chan_in_bunch_num_bytes)(bunch, &mut bytes));
            call!((self.0.chan_in_bunch_data)(bunch, &mut data));
        }
        Ok((data, bytes))
    }
    unsafe fn receive_download_data(&mut self, download: *mut UDownload, data: *const u8, bytes: i32) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_download_receive_data)(download.cast(), data, bytes)); }
        Ok(())
    }
    unsafe fn log_file_invalid(&mut self) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_log_file_net_invalid)()); }
        Ok(())
    }
    unsafe fn send_close_reply(&mut self, channel: *mut UChannel) -> Result<(), Thrown> {
        unsafe {
            net_bunch::with_out(&self.0.bunches, channel, 1, |reply| {
                let _ = channel_send_bunch(self.0, channel, reply, 0)?;
                Ok(())
            })
        }
    }
    unsafe fn package_count(&mut self, connection: *mut UNetConnection) -> Result<i32, Thrown> {
        Ok(unsafe { (*(*connection).package_map.cast::<UPackageMap>()).list.num })
    }
    unsafe fn package_info(&mut self, connection: *mut UNetConnection, index: i32) -> Result<FilePackageInfo, Thrown> {
        let mut size = 0;
        let mut url = std::ptr::null_mut();
        let mut guid = [0; 4];
        unsafe {
            call!((self.0.chan_package_info)(
                (*connection).package_map,
                index,
                &mut size,
                &mut url,
                guid.as_mut_ptr().cast(),
            ));
            let url = FileReceiveUrl::from_header(url).ok_or(Thrown)?;
            Ok(FilePackageInfo { url, guid })
        }
    }
    unsafe fn package_has_linker(&mut self, connection: *mut UNetConnection, index: i32) -> Result<bool, Thrown> {
        let mut has_linker = 0;
        unsafe {
            call!((self.0.chan_package_has_linker)(
                (*connection).package_map,
                index,
                &mut has_linker,
            ));
        }
        Ok(has_linker != 0)
    }
    unsafe fn remove_package(&mut self, connection: *mut UNetConnection, index: i32) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_package_remove)((*connection).package_map, index)); }
        Ok(())
    }
    unsafe fn set_source_filename(&mut self, channel: *mut UFileChannel, url: FileReceiveUrl, index: i32) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_file_copy_src_filename)(channel.cast(), url.header(), index)); }
        Ok(())
    }
    unsafe fn notify_sending_file(&mut self, connection: *mut UNetConnection, guid: &[u32; 4]) -> Result<bool, Thrown> {
        let mut notify = 0;
        unsafe { call!((self.0.chan_notify_sending_file)(connection.cast(), guid.as_ptr().cast_mut().cast(), &mut notify)); }
        Ok(notify != 0)
    }
    unsafe fn file_size(&mut self, url: FileReceiveUrl) -> Result<i32, Thrown> {
        let mut size = 0;
        unsafe { call!((self.0.chan_file_size)(url.header().cast::<u32>(), &mut size)); }
        Ok(size)
    }
    unsafe fn create_file_reader(&mut self, channel: *mut UFileChannel) -> Result<(), Thrown> {
        unsafe {
            call!((self.0.chan_file_create_reader)(
                (*channel).src_filename.as_ptr(),
                &mut (*channel).send_file_ar,
            ));
        }
        Ok(())
    }
    unsafe fn log_file_skip(&mut self, channel: *mut UFileChannel) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_log_file_skip)((*channel).src_filename.as_ptr())); }
        Ok(())
    }
    unsafe fn log_file_send(&mut self, channel: *mut UFileChannel) -> Result<(), Thrown> {
        unsafe { call!((self.0.chan_log_file_net_send)((*channel).src_filename.as_ptr())); }
        Ok(())
    }
    unsafe fn download_allowed(&mut self, connection: *mut UNetConnection) -> Result<bool, Thrown> {
        Ok(unsafe { (*(*connection).driver.cast::<UNetDriver>()).allow_downloads != 0 })
    }
    unsafe fn sending_file(&mut self, channel: *mut UFileChannel) -> bool {
        unsafe { !(*channel).send_file_ar.is_null() }
    }
}

/// Native adapter for the shared `UFileChannel::ReceivedBunch` policy.
///
/// # Safety
/// `ch` and `bunch` must be live and `s` must be the initialized native service table.
pub unsafe fn file_received_bunch(
    s: &Hp2NetChanServices,
    ch: *mut UFileChannel,
    bunch: *mut FInBunch,
) -> Result<(), Thrown> {
    unsafe { file_received_bunch_with(&mut NativeFileReceiveOps(s), ch, bunch) }
}
/// Core-owned counterpart to `file_received_bunch`; no native service table is used.
///
/// # Safety
/// `channel` and `bunch` must be live and the bunch must retain its packet reader.
pub unsafe fn file_received_bunch_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    channel: *mut UFileChannel,
    bunch: *mut FInBunch,
) -> hp2_core::core_access::RuntimeResult<()> {
    let mut ops = crate::file_channel_owner::CoreFileReceiveOps::new(access);
    unsafe { file_received_bunch_with(&mut ops, channel, bunch) }
}

/// Shared `UFileChannel::Tick` algorithm. File storage and packet/channel
/// operations are supplied by each owner; the scheduling and bunch sequence
/// remain one policy body.
///
/// # Safety
/// `channel`, its connection and send archive are live for the duration.
pub unsafe fn file_tick_with<O: FileTickOps>(
    channel: *mut UFileChannel,
    lan_play: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        let base = channel.cast::<UChannel>();
        channel_tick_with(base, ops)?;
        let connection = (*base).connection.cast::<UNetConnection>();
        (*connection).time_sensitive = 1;
        let mut buffer = Vec::new();
        while (*base).opened_locally == 0 && !(*channel).send_file_ar.is_null() {
            if channel_is_net_ready_with(base, lan_play, ops)? == 0 {
                break;
            }
            let mut size = channel_max_send_bytes_with(base, ops)?;
            if size == 0 {
                break;
            }
            let file_size = ops.package_file_size(connection, (*channel).package_index)?;
            let remaining = file_size - (*channel).sent_data;
            ops.with_out_bunch(base, (size >= remaining) as i32, |bunch| {
                if size > remaining {
                    size = remaining;
                }
                buffer.resize(size as usize, 0);
                ops.archive_serialize(
                    (*channel).send_file_ar,
                    buffer.as_mut_ptr(),
                    size,
                )?;
                ops.bunch_serialize_bytes(bunch, buffer.as_mut_ptr(), size)?;
                let _ = ops.archive_is_error((*channel).send_file_ar)?;
                (*channel).sent_data += size;
                (*bunch).b_reliable = 1;
                if ops.bunch_is_error(bunch)? {
                    ops.check_failed(c"!Bunch.IsError()", 1134)?;
                }
                let _ = ops.send_bunch(base, bunch)?;
                ops.flush_net(connection)?;
                if (*bunch).b_close != 0 {
                    ops.delete_archive((*channel).send_file_ar)?;
                    (*channel).send_file_ar = ptr::null_mut();
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

struct NativeFileTickOps<'a>(&'a Hp2NetChanServices);

impl ChannelTickOps for NativeFileTickOps<'_> {
    type Error = Thrown;

    unsafe fn check_failed(&self, expression: &std::ffi::CStr, line: i32) -> Result<(), Self::Error> {
        unsafe { NativeChannelTickOps(self.0).check_failed(expression, line) }
    }
    unsafe fn driver_time_since(
        &self,
        driver: *mut UNetDriver,
        time: *const u8,
    ) -> Result<f32, Self::Error> {
        unsafe { NativeChannelTickOps(self.0).driver_time_since(driver, time) }
    }
    unsafe fn send_raw_bunch(
        &self,
        connection: *mut UNetConnection,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe { NativeChannelTickOps(self.0).send_raw_bunch(connection, bunch) }
    }
    unsafe fn connection_is_net_ready(
        &self,
        connection: *mut UNetConnection,
        saturate: i32,
    ) -> Result<i32, Self::Error> {
        unsafe { NativeChannelTickOps(self.0).connection_is_net_ready(connection, saturate) }
    }
    unsafe fn connection_out_num_bits(
        &self,
        connection: *mut UNetConnection,
    ) -> Result<i32, Self::Error> {
        unsafe { NativeChannelTickOps(self.0).connection_out_num_bits(connection) }
    }
}

impl FileTickOps for NativeFileTickOps<'_> {
    unsafe fn package_file_size(
        &self,
        connection: *mut UNetConnection,
        package_index: i32,
    ) -> Result<i32, Self::Error> {
        unsafe {
            let mut file_size = 0;
            let mut unused = 0;
            call!((self.0.chan_package_file_info)(
                (*connection).package_map,
                package_index,
                &mut file_size,
                &mut unused,
            ));
            Ok(file_size)
        }
    }

    unsafe fn archive_serialize(
        &self,
        archive: *mut c_void,
        data: *mut u8,
        count: i32,
    ) -> Result<(), Self::Error> {
        unsafe {
            call!((self.0.chan_file_archive_serialize)(archive, data, count));
            Ok(())
        }
    }

    unsafe fn archive_is_error(&self, archive: *mut c_void) -> Result<bool, Self::Error> {
        unsafe {
            let mut is_error = 0;
            call!((self.0.chan_file_archive_is_error)(archive, &mut is_error));
            Ok(is_error != 0)
        }
    }

    unsafe fn bunch_serialize_bytes(
        &self,
        bunch: *mut FOutBunch,
        data: *mut u8,
        count: i32,
    ) -> Result<(), Self::Error> {
        unsafe {
            call!((self.0.chan_out_bunch_serialize_bytes)(bunch, data, count));
            Ok(())
        }
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Self::Error> {
        unsafe {
            let mut is_error = 0;
            call!((self.0.chan_out_bunch_is_error)(bunch, &mut is_error));
            Ok(is_error != 0)
        }
    }

    unsafe fn with_out_bunch<F>(
        &self,
        channel: *mut UChannel,
        close: i32,
        body: F,
    ) -> Result<(), Self::Error>
    where
        F: FnOnce(*mut FOutBunch) -> Result<(), Self::Error>,
    {
        unsafe { net_bunch::with_out(&self.0.bunches, channel, close, body) }
    }

    unsafe fn send_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe { channel_send_bunch(self.0, channel, bunch, 0).map(|_| ()) }
    }

    unsafe fn flush_net(&self, connection: *mut UNetConnection) -> Result<(), Self::Error> {
        unsafe {
            call!((self.0.chan_conn_flush_net)(connection.cast()));
            Ok(())
        }
    }

    unsafe fn delete_archive(&self, archive: *mut c_void) -> Result<(), Self::Error> {
        unsafe {
            call!((self.0.chan_file_archive_delete)(archive));
            Ok(())
        }
    }
}

/// Native adapter for the shared `UFileChannel::Tick` algorithm.
///
/// # Safety
/// Pointer arguments and the initialized services table are live.
pub unsafe fn file_tick(
    services: &Hp2NetChanServices,
    channel: *mut UFileChannel,
    lan_play: i32,
) -> Result<(), Thrown> {
    unsafe { file_tick_with(channel, lan_play, &NativeFileTickOps(services)) }
}

///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn file_destroy(s: &Hp2NetChanServices, ch: *mut UFileChannel) -> Result<(), Thrown> {
    unsafe {
        let base = ch.cast::<UChannel>();
        if (*base).connection.is_null() {
            checked!(s, "Connection", 1149);
        }
        if channel_route_destroy(s, base)? != 0 {
            return Ok(());
        }
        if !slot_is(base) {
            checked!(s, "Connection->Channels[ChIndex]==this", 1152);
        }
        if !(*ch).send_file_ar.is_null() {
            call!((s.chan_file_archive_delete)((*ch).send_file_ar));
            (*ch).send_file_ar = std::ptr::null_mut();
        }
        if (*base).opened_locally != 0 && !(*ch).download.is_null() {
            call!((s.chan_download_done)((*ch).download));
            call!((s.chan_download_delete)((*ch).download));
            (*ch).download = std::ptr::null_mut();
        }
        channel_destroy(s, base)?;
        Ok(())
    }
}
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn file_describe(
    s: &Hp2NetChanServices,
    ch: *mut UFileChannel,
    out: *mut c_void,
) -> Result<(), Thrown> {
    unsafe { describe_file_with(&mut NativeDescriptionOps { services: s, output: out }, ch) }
}

pub(crate) trait ChannelDescriptionOps {
    type Error;
    fn set_text(&mut self, text: &[u32]) -> Result<(), Self::Error>;
    fn append_text(&mut self, text: &[u32]) -> Result<(), Self::Error>;
    unsafe fn set_actor_prefix(&mut self, actor: *mut AActor) -> Result<(), Self::Error>;
    unsafe fn package_file_size(&mut self, connection: *mut UNetConnection, index: i32) -> Result<i32, Self::Error>;
    unsafe fn set_file_prefix(&mut self, filename: *const u32, receiving: bool, transferred: i32, size: i32) -> Result<(), Self::Error>;
}

struct NativeDescriptionOps<'a> {
    services: &'a Hp2NetChanServices,
    output: *mut c_void,
}

impl ChannelDescriptionOps for NativeDescriptionOps<'_> {
    type Error = Thrown;
    fn set_text(&mut self, text: &[u32]) -> Result<(), Thrown> {
        unsafe { call!((self.services.chan_string_set_tchar)(self.output, text.as_ptr())); }
        Ok(())
    }
    fn append_text(&mut self, text: &[u32]) -> Result<(), Thrown> {
        unsafe { net_bunch::with_string(|temporary| {
            unsafe {
                call!((self.services.chan_string_set_tchar)(temporary, text.as_ptr()));
                call!((self.services.chan_string_append)(self.output, temporary));
            }
            Ok(())
        }) }
    }
    unsafe fn set_actor_prefix(&mut self, actor: *mut AActor) -> Result<(), Thrown> {
        let mut role = 0;
        let mut remote_role = 0;
        unsafe {
            call!((self.services.chan_actor_get_roles)(actor.cast(), &mut role, &mut remote_role));
            call!((self.services.chan_actor_describe_format)(self.output, actor.cast(), role, remote_role));
        }
        Ok(())
    }
    unsafe fn package_file_size(&mut self, connection: *mut UNetConnection, index: i32) -> Result<i32, Thrown> {
        let mut size = 0;
        let mut flags = 0;
        unsafe { call!((self.services.chan_package_file_info)((*connection).package_map, index, &mut size, &mut flags)); }
        Ok(size)
    }
    unsafe fn set_file_prefix(&mut self, filename: *const u32, receiving: bool, transferred: i32, size: i32) -> Result<(), Thrown> {
        let mut text = Vec::new();
        unsafe { append_file_description(&mut text, filename, receiving, transferred, size); }
        text.push(0);
        self.set_text(&text)
    }
}

fn description_state(channel: *mut UChannel) -> &'static [u32] {
    static OPEN: [u32; 12] = [83, 116, 97, 116, 101, 61, 111, 112, 101, 110, 0, 0];
    static CLOSING: [u32; 15] = [83, 116, 97, 116, 101, 61, 99, 108, 111, 115, 105, 110, 103, 0, 0];
    if unsafe { (*channel).closing != 0 } { &CLOSING } else { &OPEN }
}

/// # Safety
/// Channel and output backend are live.
pub(crate) unsafe fn describe_base_with<O: ChannelDescriptionOps>(ops: &mut O, channel: *mut UChannel) -> Result<(), O::Error> {
    ops.set_text(description_state(channel))
}

/// # Safety
/// Channel and output backend are live.
pub(crate) unsafe fn describe_control_with<O: ChannelDescriptionOps>(ops: &mut O, channel: *mut UChannel) -> Result<(), O::Error> {
    ops.set_text(&[84, 101, 120, 116, 32, 0])?;
    ops.append_text(description_state(channel))
}

/// # Safety
/// Channel, its Actor when present, and output backend are live.
pub(crate) unsafe fn describe_actor_with<O: ChannelDescriptionOps>(ops: &mut O, channel: *mut UActorChannel) -> Result<(), O::Error> {
    let actor = unsafe { (*channel).actor.cast::<AActor>() };
    if unsafe { (*channel.cast::<UChannel>()).closing != 0 } || actor.is_null() {
        ops.set_text(&[65, 99, 116, 111, 114, 61, 78, 111, 110, 101, 32, 0])?;
    } else {
        unsafe { ops.set_actor_prefix(actor)?; }
    }
    ops.append_text(description_state(channel.cast()))
}

pub(crate) fn append_description_number(output: &mut Vec<u32>, value: i32) {
    if value < 0 { output.push(u32::from(b'-')); }
    let mut number = value.unsigned_abs();
    let mut digits = [0; 10];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = u32::from(b'0') + number % 10;
        number /= 10;
        if number == 0 { break; }
    }
    output.extend_from_slice(&digits[start..]);
}

/// Appends the source file-channel prefix without allocating another text buffer.
///
/// # Safety
/// A non-null filename is readable through its first NUL.
pub(crate) unsafe fn append_file_description(output: &mut Vec<u32>, filename: *const u32, receiving: bool, transferred: i32, size: i32) {
    let filename = if filename.is_null() { &[][..] } else {
        let mut count = 0;
        unsafe {
            while *filename.add(count) != 0 { count += 1; }
            std::slice::from_raw_parts(filename, count)
        }
    };
    output.reserve(filename.len() + 53);
    output.extend(b"File='".iter().copied().map(u32::from));
    output.extend_from_slice(filename);
    output.extend(b"', ".iter().copied().map(u32::from));
    let status = if receiving { &b"Received="[..] } else { &b"Sent="[..] };
    output.extend(status.iter().copied().map(u32::from));
    append_description_number(output, transferred);
    output.push(u32::from(b'/'));
    append_description_number(output, size);
    output.push(u32::from(b' '));
}

/// # Safety
/// Channel, connection, package entry and download when present are live.
pub(crate) unsafe fn describe_file_with<O: ChannelDescriptionOps>(ops: &mut O, channel: *mut UFileChannel) -> Result<(), O::Error> {
    unsafe {
        let base = channel.cast::<UChannel>();
        let connection = (*base).connection.cast::<UNetConnection>();
        let size = ops.package_file_size(connection, (*channel).package_index)?;
        let receiving = (*base).opened_locally != 0;
        let download = (*channel).download.cast::<UDownload>();
        let filename = if receiving {
            if download.is_null() { ptr::null() } else { (*download).temp_filename.as_ptr() }
        } else { (*channel).src_filename.as_ptr() };
        let transferred = if receiving {
            if download.is_null() { 0 } else { (*download).transfered }
        } else { (*channel).sent_data };
        ops.set_file_prefix(filename, receiving, transferred, size)?;
        ops.append_text(description_state(base))
    }
}
