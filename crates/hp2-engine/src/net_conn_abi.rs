//! ABI mirror of `Hp2NetConnServices` in `Engine/Inc/Hp2NetConnServices.h`.
//!
//! `S` thunks return zero after completion and nonzero after a captured C++
//! exception. `K` thunks are non-throwing direct leaf operations.

use crate::net_bunch::Hp2NetBunchServices;
use hp2_abi::engine::generated::{
    FBitReader, FBitWriter, FInBunch, FOutBunch, UChannel, UNetConnection, UPackageMap,
};
use std::{ffi::{c_char, c_void}, mem::{offset_of, size_of}, ptr};

/// A C++ exception captured by a services thunk.
///
/// This is the same capture marker used by the channel/bunch adapter: native
/// connection packet operations hand their scoped bunches to
/// [`crate::net_bunch`], whose leaves report the same captured exception.
pub use crate::net_chan::Thrown;

/// Result of an operation that can invoke a throwing services thunk.
pub type Res<T> = Result<T, Thrown>;
/// Packet operations shared by the native adapter and canonical Rust owners.
///
/// The associated error preserves each caller's real error channel: legacy
/// calls retain the captured C++ exception marker; canonical calls carry
/// `RuntimeError` directly.
pub trait ConnectionPacketOps {
    type Error;

    unsafe fn check_fail(&self, expression: &std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn out(&self, conn: *mut c_void) -> *mut c_void;
    unsafe fn writer_num_bits(&self, writer: *mut c_void) -> i32;
    unsafe fn writer_num_bytes(&self, writer: *mut c_void) -> i32;
    unsafe fn writer_data(&self, writer: *mut c_void) -> *mut c_void;
    unsafe fn writer_is_error(&self, writer: *mut c_void) -> bool;
    unsafe fn writer_write_bit(&self, writer: *mut c_void, bit: i32) -> Result<(), Self::Error>;
    unsafe fn writer_write_int(&self, writer: *mut c_void, value: i32, max: i32) -> Result<(), Self::Error>;
    unsafe fn writer_serialize_bits(&self, writer: *mut c_void, data: *const c_void, bits: i32) -> Result<(), Self::Error>;
    unsafe fn reset_last_end(&self, conn: *mut c_void);
    unsafe fn mark_last_start(&self, conn: *mut c_void);
    unsafe fn pre_send_overflow(&self, num_bits: i32, size_bits: i32, max_bits: i32) -> Result<(), Self::Error>;
    unsafe fn low_level_send(&self, conn: *mut c_void, data: *const c_void, bytes: i32) -> Result<(), Self::Error>;
    fn random(&self) -> f32;
    fn ceil_log_two(&self, value: u32) -> i32;
    fn seconds(&self) -> i64;
    unsafe fn init_out(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn flush_net(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn queued_ack_count(&self, conn: *mut c_void) -> i32;
    unsafe fn queued_ack(&self, conn: *mut c_void, index: i32) -> Result<i32, Self::Error>;
    unsafe fn add_queued_ack(&self, conn: *mut c_void, ack: i32) -> Result<(), Self::Error>;
    unsafe fn resend_ack_count(&self, conn: *mut c_void) -> i32;
    unsafe fn resend_ack(&self, conn: *mut c_void, index: i32) -> Result<i32, Self::Error>;
    unsafe fn add_resend_ack(&self, conn: *mut c_void, ack: i32) -> Result<(), Self::Error>;
    unsafe fn empty_queued_acks(&self, conn: *mut c_void, slack: i32) -> Result<(), Self::Error>;
    unsafe fn empty_resend_acks(&self, conn: *mut c_void, slack: i32) -> Result<(), Self::Error>;
    unsafe fn delayed_add(&self, conn: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn delayed_at(&self, conn: *mut c_void, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn delayed_data_add(&self, packet: *mut c_void, bytes: i32) -> Result<(), Self::Error>;
    unsafe fn delayed_data(&self, packet: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn delayed_data_num(&self, packet: *mut c_void) -> i32;
    unsafe fn delayed_remove(&self, conn: *mut c_void, index: i32) -> Result<(), Self::Error>;
    unsafe fn writer_construct(&self, writer: *mut FBitWriter) -> Result<(), Self::Error>;
    unsafe fn writer_destruct(&self, writer: *mut FBitWriter) -> Result<(), Self::Error>;
    unsafe fn writer_initialize(&self, writer: *mut FBitWriter, capacity: i32) -> Result<(), Self::Error>;
    unsafe fn writer_release_buffer(&self, writer: *mut FBitWriter) -> Result<(), Self::Error>;
    unsafe fn out_bunch_is_error(&self, bunch: *mut FOutBunch) -> bool;
}
/// Packet receive leaves used by both native shipping and Core records.
///
/// This adapter contains reader/channel primitives; `net_conn` owns the
/// packet ordering, acknowledgement and bunch state machines.
pub(crate) trait ConnectionReceiveOps: ConnectionPacketOps {
    unsafe fn reader_construct(&self, reader: *mut FBitReader) -> Result<(), Self::Error>;
    unsafe fn reader_initialize(
        &self,
        reader: *mut FBitReader,
        data: *const u8,
        bits: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn reader_release(&self, reader: *mut FBitReader) -> Result<(), Self::Error>;
    unsafe fn reader_destruct(&self, reader: *mut FBitReader) -> Result<(), Self::Error>;
    unsafe fn reader_is_error(&self, reader: *mut FBitReader) -> bool;
    unsafe fn reader_at_end(&self, reader: *mut FBitReader) -> bool;
    unsafe fn reader_read_bit(&self, reader: *mut FBitReader) -> i32;
    unsafe fn reader_read_int(&self, reader: *mut FBitReader, max: i32) -> i32;
    unsafe fn bunch_construct(&self, bunch: *mut FInBunch) -> Result<(), Self::Error>;
    unsafe fn bunch_destruct(&self, bunch: *mut FInBunch) -> Result<(), Self::Error>;
    unsafe fn bunch_release(&self, bunch: *mut FInBunch) -> Result<(), Self::Error>;
    unsafe fn bunch_resize_buffer(
        &self,
        bunch: *mut FInBunch,
        bytes: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn open_channel_count(&self, conn: *mut c_void) -> i32;
    unsafe fn open_channel_at(
        &self,
        conn: *mut c_void,
        index: i32,
    ) -> Result<*mut UChannel, Self::Error>;
    unsafe fn known_channel_type(&self, channel_type: i32) -> Result<bool, Self::Error>;
    unsafe fn construct_channel(
        &self,
        conn: *mut c_void,
        channel_type: i32,
    ) -> Result<*mut UChannel, Self::Error>;
    unsafe fn initialize_channel(
        &self,
        channel_type: i32,
        channel: *mut UChannel,
        conn: *mut c_void,
        index: i32,
        opened_locally: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn add_open_channel(
        &self,
        conn: *mut c_void,
        channel: *mut UChannel,
    ) -> Result<(), Self::Error>;
    unsafe fn notify_accepting_channel(
        &self,
        conn: *mut c_void,
        channel: *mut UChannel,
    ) -> Result<bool, Self::Error>;
    unsafe fn channel_received_nak(
        &self,
        channel: *mut UChannel,
        packet_id: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn channel_received_acks(&self, channel: *mut UChannel) -> Result<(), Self::Error>;
    unsafe fn channel_received_raw(
        &self,
        channel: *mut UChannel,
        bunch: *mut FInBunch,
    ) -> Result<(), Self::Error>;
    unsafe fn with_close_bunch<T>(
        &self,
        channel: *mut UChannel,
        body: impl FnOnce(*mut FOutBunch) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    unsafe fn channel_send_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error>;
    unsafe fn delete_channel(&self, channel: *mut UChannel) -> Result<(), Self::Error>;
}
/// Per-frame connection tick leaves shared by native services and Core owners.
pub(crate) trait ConnectionTickOps: ConnectionReceiveOps {
    fn app_round(&self, value: f32) -> i32;
    unsafe fn log_timeout(&self, timeout: f32, elapsed: f32) -> Result<(), Self::Error>;
    unsafe fn actor_channel_exists(
        &self,
        connection: *mut UNetConnection,
        actor: *mut c_void,
    ) -> Result<bool, Self::Error>;
    unsafe fn set_bad_connection_alert(&self, actor: *mut c_void, value: bool);
    unsafe fn channel_tick(&self, channel: *mut UChannel) -> Result<(), Self::Error>;
    unsafe fn download_tick(&self, download: *mut c_void) -> Result<(), Self::Error>;
}

/// ReceiveFile sequencing shared by the native leaf adapter and Core owner.
pub(crate) trait ConnectionReceiveFileOps {
    type Error;
    unsafe fn check_fail(&self, expression: &std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn package_count(&self, conn: *mut UNetConnection) -> i32;
    unsafe fn download_info_count(&self, conn: *mut UNetConnection) -> i32;
    unsafe fn add_default_download_info(&self, conn: *mut UNetConnection) -> Result<(), Self::Error>;
    unsafe fn delete_download(&self, download: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn construct_download(
        &self,
        conn: *mut UNetConnection,
    ) -> Result<*mut c_void, Self::Error>;
    unsafe fn receive_file(
        &self,
        conn: *mut UNetConnection,
        package_index: i32,
        params: *const u32,
        compression: i32,
    ) -> Result<(), Self::Error>;
}

/// Source-order connection destruction hooks shared by native and Core owners.
pub trait ConnectionDestroyOps {
    type Error;

    unsafe fn log_close(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn close_channel_zero(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn flush_net(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn unregister(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn set_closed(&self, conn: *mut c_void);
    unsafe fn delete_open_channels(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn delete_package_map(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn delete_download(&self, conn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn super_destroy(&self, conn: *mut c_void) -> Result<(), Self::Error>;
}


/// Converts a services status into the Rust-side exception protocol.
#[inline]
pub fn done(status: i32) -> Res<()> {
    if status == 0 { Ok(()) } else { Err(Thrown) }
}

/// C++ leaf callbacks reached by the currently Rust-owned connection methods.
/// Field order and signatures exactly mirror `Hp2NetConnServices`.
#[repr(C)]
pub struct Hp2NetConnServices {
    // Assertion, package-map, and integer formatting leaves.
    pub conn_check_fail: unsafe extern "C" fn(*const c_char, *const c_void, i32) -> i32,
    pub app_ceil_log_two: unsafe extern "C" fn(u32) -> i32,
    pub log_close: unsafe extern "C" fn(*mut c_void) -> i32,
    pub ar_log_int: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    // Connection virtual calls and fields.
    pub conn_super_serialize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub conn_super_destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    pub conn_super_exec:
        unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void, *mut i32) -> i32,
    pub conn_parse_command: unsafe extern "C" fn(*mut *const c_void, i32) -> i32,
    pub conn_flush_net: unsafe extern "C" fn(*mut c_void) -> i32,
    pub conn_send_ack: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    pub conn_out_reset: unsafe extern "C" fn(*mut c_void) -> i32,
    pub conn_out: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    // Channel forwarding leaves.
    pub chan_close: unsafe extern "C" fn(*mut c_void) -> i32,
    pub chan_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    pub chan_received_nak: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub chan_received_acks: unsafe extern "C" fn(*mut c_void) -> i32,
    // Array operations.
    pub arr_ptr_num: unsafe extern "C" fn(*mut c_void) -> i32,
    pub arr_ptr_get: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub arr_i32_num: unsafe extern "C" fn(*mut c_void) -> i32,
    pub arr_i32_get: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub arr_i32_add: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub arr_i32_empty: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    // Packet writer leaves.
    pub writer_num_bits: unsafe extern "C" fn(*mut c_void) -> i32,
    pub writer_write_bit: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub writer_write_int: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    // Driver and download-manager leaves.
    pub driver_remove_client: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub dlmgr_load_class: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub dlmgr_log_defaults: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    // Package-map leaves.
    pub pkg_log_uses: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub pkg_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    // Client and pawn leaves.
    pub pawn_client: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub client_viewport_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub client_viewport0: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub viewport_clear_actor_player: unsafe extern "C" fn(*mut c_void),
    pub viewport_set_net_speed: unsafe extern "C" fn(*mut c_void, i32),
    pub pawn_set_role: unsafe extern "C" fn(*mut c_void, i32),
    pub pawn_set_show_flags: unsafe extern "C" fn(*mut c_void, u32),
    pub pawn_set_rend_map: unsafe extern "C" fn(*mut c_void, i32),
    pub pawn_set_player: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub pawn_reset_input: unsafe extern "C" fn(*mut c_void) -> i32,
    pub pawn_set_level_action: unsafe extern "C" fn(*mut c_void, i32),
    // Download information/object leaves.
    pub dl_info_num: unsafe extern "C" fn(*mut c_void) -> i32,
    pub dl_info_add_default: unsafe extern "C" fn(*mut c_void) -> i32,
    pub dl_construct: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub dl_receive_file: unsafe extern "C" fn(*mut c_void, i32, *const u32, i32) -> i32,
    pub dl_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    // Archive and packet-overflow leaves.
    pub ar_serialize_package_map: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub ar_serialize_channel: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub ar_serialize_download: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub conn_pre_send_overflow: unsafe extern "C" fn(i32, i32, i32) -> i32,
    pub app_round: unsafe extern "C" fn(f32) -> i32,
    // Time, random, and log leaves.
    pub app_seconds: unsafe extern "C" fn() -> i64,
    pub app_frand: unsafe extern "C" fn() -> f32,
    pub conn_log_timeout: unsafe extern "C" fn(f32, f32) -> i32,
    // Connection transport and state leaves.
    pub conn_low_level_send: unsafe extern "C" fn(*mut c_void, *const c_void, i32) -> i32,
    pub conn_mark_last_start: unsafe extern "C" fn(*mut c_void),
    pub conn_reset_last_end: unsafe extern "C" fn(*mut c_void),
    pub control_chan_serialize: unsafe extern "C" fn(*mut c_void, *const c_void, i32) -> i32,
    // Bit writer extension leaves.
    pub writer_num_bytes: unsafe extern "C" fn(*mut c_void) -> i32,
    pub writer_data: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub writer_is_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub writer_serialize_bits: unsafe extern "C" fn(*mut c_void, *const c_void, i32) -> i32,
    pub writer_construct_identity: unsafe extern "C" fn(*mut FBitWriter) -> i32,
    pub writer_destruct_identity: unsafe extern "C" fn(*mut FBitWriter) -> i32,
    // Bit reader leaves.
    pub reader_construct_identity: unsafe extern "C" fn(*mut FBitReader) -> i32,
    pub reader_destruct_identity: unsafe extern "C" fn(*mut FBitReader) -> i32,
    pub reader_is_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub reader_at_end: unsafe extern "C" fn(*mut c_void) -> i32,
    pub reader_get_pos: unsafe extern "C" fn(*mut c_void) -> i32,
    pub reader_read_bit: unsafe extern "C" fn(*mut c_void) -> i32,
    pub reader_read_int: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    // Bunch leaves.
    pub bunches: Hp2NetBunchServices,
    pub out_bunch_is_error: unsafe extern "C" fn(*mut c_void) -> i32,
    // Channel leaves.
    pub chan_is_known_type: unsafe extern "C" fn(i32) -> i32,
    pub chan_received_raw: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub chan_send_bunch: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub chan_tick: unsafe extern "C" fn(*mut c_void) -> i32,
    pub chan_init: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32) -> i32,
    pub conn_construct_channel: unsafe extern "C" fn(i32, *mut *mut c_void) -> i32,
    // Driver, actor, array, and delayed-packet leaves.
    pub driver_notify_accepting: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub conn_actor_channel_find: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void,
    pub actor_set_bad_connection_alert: unsafe extern "C" fn(*mut c_void, i32),
    pub arr_ptr_add: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub delayed_add: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub delayed_elem: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub delayed_data_add: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub delayed_data_ptr: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub delayed_data_num: unsafe extern "C" fn(*mut c_void) -> i32,
    pub delayed_remove: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    // Download tick leaf.
    pub dl_tick: unsafe extern "C" fn(*mut c_void) -> i32,
}

impl ConnectionPacketOps for Hp2NetConnServices {
    type Error = Thrown;

    unsafe fn check_fail(&self, expression: &std::ffi::CStr, line: i32) -> Res<()> {
        // SAFETY: the native adapter owns the static filename and exception boundary.
        unsafe { done((self.conn_check_fail)(expression.as_ptr(), c"UnConn.cpp".as_ptr().cast(), line)) }
    }
    unsafe fn out(&self, conn: *mut c_void) -> *mut c_void {
        // SAFETY: caller supplies a live UNetConnection.
        unsafe { (self.conn_out)(conn) }
    }
    unsafe fn writer_num_bits(&self, writer: *mut c_void) -> i32 {
        // SAFETY: writer is an initialized FBitWriter or FOutBunch archive.
        unsafe { (self.writer_num_bits)(writer) }
    }
    unsafe fn writer_num_bytes(&self, writer: *mut c_void) -> i32 {
        // SAFETY: writer is an initialized FBitWriter.
        unsafe { (self.writer_num_bytes)(writer) }
    }
    unsafe fn writer_data(&self, writer: *mut c_void) -> *mut c_void {
        // SAFETY: writer is an initialized FBitWriter.
        unsafe { (self.writer_data)(writer) }
    }
    unsafe fn writer_is_error(&self, writer: *mut c_void) -> bool {
        // SAFETY: writer is an initialized FBitWriter.
        unsafe { (self.writer_is_error)(writer) != 0 }
    }
    unsafe fn writer_write_bit(&self, writer: *mut c_void, bit: i32) -> Res<()> {
        // SAFETY: writer is an initialized FBitWriter.
        unsafe { done((self.writer_write_bit)(writer, bit)) }
    }
    unsafe fn writer_write_int(&self, writer: *mut c_void, value: i32, max: i32) -> Res<()> {
        // SAFETY: writer is an initialized FBitWriter.
        unsafe { done((self.writer_write_int)(writer, value, max)) }
    }
    unsafe fn writer_serialize_bits(&self, writer: *mut c_void, data: *const c_void, bits: i32) -> Res<()> {
        // SAFETY: both archives are live and data covers the requested bit range.
        unsafe { done((self.writer_serialize_bits)(writer, data, bits)) }
    }
    unsafe fn reset_last_end(&self, conn: *mut c_void) {
        // SAFETY: conn is a live UNetConnection.
        unsafe { (self.conn_reset_last_end)(conn) }
    }
    unsafe fn mark_last_start(&self, conn: *mut c_void) {
        // SAFETY: conn is a live UNetConnection.
        unsafe { (self.conn_mark_last_start)(conn) }
    }
    unsafe fn pre_send_overflow(&self, num_bits: i32, size_bits: i32, max_bits: i32) -> Res<()> {
        // SAFETY: this native check thunk is exception guarded.
        unsafe { done((self.conn_pre_send_overflow)(num_bits, size_bits, max_bits)) }
    }
    unsafe fn low_level_send(&self, conn: *mut c_void, data: *const c_void, bytes: i32) -> Res<()> {
        // SAFETY: conn is live and data points to bytes readable packet bytes.
        unsafe { done((self.conn_low_level_send)(conn, data, bytes)) }
    }
    fn random(&self) -> f32 {
        // SAFETY: native appFrand is a process-global random source.
        unsafe { (self.app_frand)() }
    }
    fn ceil_log_two(&self, value: u32) -> i32 {
        unsafe { (self.app_ceil_log_two)(value) }
    }
    fn seconds(&self) -> i64 {
        // SAFETY: native appSeconds returns its raw FTime representation.
        unsafe { (self.app_seconds)() }
    }
    unsafe fn init_out(&self, conn: *mut c_void) -> Res<()> {
        // SAFETY: conn is live.
        unsafe { done((self.conn_out_reset)(conn)) }
    }
    unsafe fn flush_net(&self, conn: *mut c_void) -> Res<()> {
        // SAFETY: virtual dispatch is retained only for this legacy native adapter.
        unsafe { done((self.conn_flush_net)(conn)) }
    }
    unsafe fn queued_ack_count(&self, conn: *mut c_void) -> i32 {
        unsafe { (self.arr_i32_num)(std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).queued_acks).cast()) }
    }
    unsafe fn queued_ack(&self, conn: *mut c_void, index: i32) -> Res<i32> {
        let array = unsafe { std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).queued_acks).cast() };
        let mut value = 0;
        unsafe { done((self.arr_i32_get)(array, index, &mut value))?; }
        Ok(value)
    }
    unsafe fn add_queued_ack(&self, conn: *mut c_void, ack: i32) -> Res<()> {
        let array = unsafe { std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).queued_acks).cast() };
        unsafe { done((self.arr_i32_add)(array, ack)) }
    }
    unsafe fn resend_ack_count(&self, conn: *mut c_void) -> i32 {
        unsafe { (self.arr_i32_num)(std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).resend_acks).cast()) }
    }
    unsafe fn resend_ack(&self, conn: *mut c_void, index: i32) -> Res<i32> {
        let array = unsafe { std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).resend_acks).cast() };
        let mut value = 0;
        unsafe { done((self.arr_i32_get)(array, index, &mut value))?; }
        Ok(value)
    }
    unsafe fn add_resend_ack(&self, conn: *mut c_void, ack: i32) -> Res<()> {
        let array = unsafe { std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).resend_acks).cast() };
        unsafe { done((self.arr_i32_add)(array, ack)) }
    }
    unsafe fn empty_queued_acks(&self, conn: *mut c_void, slack: i32) -> Res<()> {
        let array = unsafe { std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).queued_acks).cast() };
        unsafe { done((self.arr_i32_empty)(array, slack)) }
    }
    unsafe fn empty_resend_acks(&self, conn: *mut c_void, slack: i32) -> Res<()> {
        let array = unsafe { std::ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).resend_acks).cast() };
        unsafe { done((self.arr_i32_empty)(array, slack)) }
    }
    unsafe fn delayed_add(&self, conn: *mut c_void) -> Res<*mut c_void> {
        let mut packet = std::ptr::null_mut();
        // SAFETY: conn is live and delayed_add returns a new initialized delayed packet.
        unsafe { done((self.delayed_add)(conn, &mut packet))?; }
        Ok(packet)
    }
    unsafe fn delayed_at(&self, conn: *mut c_void, index: i32) -> Res<*mut c_void> {
        let mut packet = std::ptr::null_mut();
        // SAFETY: the checked native array accessor validates the index.
        unsafe { done((self.delayed_elem)(conn, index, &mut packet))?; }
        Ok(packet)
    }
    unsafe fn delayed_data_add(&self, packet: *mut c_void, bytes: i32) -> Res<()> {
        // SAFETY: packet is a live DelayedPacket.
        unsafe { done((self.delayed_data_add)(packet, bytes)) }
    }
    unsafe fn delayed_data(&self, packet: *mut c_void) -> Res<*mut c_void> {
        let mut data = std::ptr::null_mut();
        // SAFETY: packet is a live DelayedPacket.
        unsafe { done((self.delayed_data_ptr)(packet, &mut data))?; }
        Ok(data)
    }
    unsafe fn delayed_data_num(&self, packet: *mut c_void) -> i32 {
        // SAFETY: packet is live.
        unsafe { (self.delayed_data_num)(packet) }
    }
    unsafe fn delayed_remove(&self, conn: *mut c_void, index: i32) -> Res<()> {
        // SAFETY: conn is live and the index was obtained from its current delayed array.
        unsafe { done((self.delayed_remove)(conn, index)) }
    }
    unsafe fn writer_construct(&self, writer: *mut FBitWriter) -> Res<()> {
        // SAFETY: writer is uninitialized storage for a native FBitWriter identity.
        unsafe { done((self.writer_construct_identity)(writer)) }
    }
    unsafe fn writer_destruct(&self, writer: *mut FBitWriter) -> Res<()> {
        // SAFETY: writer was initialized by writer_construct.
        unsafe { done((self.writer_destruct_identity)(writer)) }
    }
    unsafe fn writer_initialize(&self, writer: *mut FBitWriter, capacity: i32) -> Res<()> {
        // SAFETY: writer is an empty initialized identity archive.
        unsafe { crate::net_bunch::initialize_writer(writer, capacity) }.map_err(|_| Thrown)
    }
    unsafe fn writer_release_buffer(&self, writer: *mut FBitWriter) -> Res<()> {
        // SAFETY: writer's Core-owned buffer is detached by the shared bunch allocator.
        unsafe { crate::net_bunch::release_buffer(&raw mut (*writer).buffer) }.map_err(|_| Thrown)
    }
    unsafe fn out_bunch_is_error(&self, bunch: *mut FOutBunch) -> bool {
        // SAFETY: bunch is a live initialized FOutBunch.
        unsafe { (self.out_bunch_is_error)(bunch.cast()) != 0 }
    }
}
impl ConnectionReceiveOps for Hp2NetConnServices {
    unsafe fn reader_construct(&self, reader: *mut FBitReader) -> Res<()> {
        unsafe { done((self.reader_construct_identity)(reader)) }
    }
    unsafe fn reader_initialize(
        &self,
        reader: *mut FBitReader,
        data: *const u8,
        bits: i32,
    ) -> Res<()> {
        unsafe { crate::net_bunch::initialize_reader(reader, data, bits) }.map_err(|_| Thrown)
    }
    unsafe fn reader_release(&self, reader: *mut FBitReader) -> Res<()> {
        unsafe { crate::net_bunch::release_buffer(&raw mut (*reader).buffer) }
            .map_err(|_| Thrown)
    }
    unsafe fn reader_destruct(&self, reader: *mut FBitReader) -> Res<()> {
        unsafe { done((self.reader_destruct_identity)(reader)) }
    }
    unsafe fn reader_is_error(&self, reader: *mut FBitReader) -> bool {
        unsafe { (self.reader_is_error)(reader.cast()) != 0 }
    }
    unsafe fn reader_at_end(&self, reader: *mut FBitReader) -> bool {
        unsafe { (self.reader_at_end)(reader.cast()) != 0 }
    }
    unsafe fn reader_read_bit(&self, reader: *mut FBitReader) -> i32 {
        unsafe { (self.reader_read_bit)(reader.cast()) }
    }
    unsafe fn reader_read_int(&self, reader: *mut FBitReader, max: i32) -> i32 {
        unsafe { (self.reader_read_int)(reader.cast(), max) }
    }
    unsafe fn bunch_construct(&self, bunch: *mut FInBunch) -> Res<()> {
        unsafe { done((self.bunches.in_construct)(bunch)) }
    }
    unsafe fn bunch_destruct(&self, bunch: *mut FInBunch) -> Res<()> {
        unsafe { done((self.bunches.in_destruct)(bunch)) }
    }
    unsafe fn bunch_release(&self, bunch: *mut FInBunch) -> Res<()> {
        unsafe { crate::net_bunch::release_buffer(&raw mut (*bunch).base.buffer) }
            .map_err(|_| Thrown)
    }
    unsafe fn bunch_resize_buffer(&self, bunch: *mut FInBunch, bytes: i32) -> Res<()> {
        unsafe { crate::net_bunch::resize_buffer(&raw mut (*bunch).base.buffer, bytes) }
            .map_err(|_| Thrown)
    }
    unsafe fn open_channel_count(&self, conn: *mut c_void) -> i32 {
        unsafe {
            (self.arr_ptr_num)(
                ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).open_channels)
                    .cast(),
            )
        }
    }
    unsafe fn open_channel_at(&self, conn: *mut c_void, index: i32) -> Res<*mut UChannel> {
        let array = unsafe {
            ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).open_channels)
                .cast()
        };
        let mut channel = ptr::null_mut();
        unsafe { done((self.arr_ptr_get)(array, index, &mut channel))?; }
        Ok(channel.cast())
    }
    unsafe fn known_channel_type(&self, channel_type: i32) -> Res<bool> {
        Ok(unsafe { (self.chan_is_known_type)(channel_type) != 0 })
    }
    unsafe fn construct_channel(&self, _conn: *mut c_void, channel_type: i32) -> Res<*mut UChannel> {
        let mut channel = ptr::null_mut();
        unsafe { done((self.conn_construct_channel)(channel_type, &mut channel))?; }
        Ok(channel.cast())
    }
    unsafe fn initialize_channel(
        &self,
        _channel_type: i32,
        channel: *mut UChannel,
        conn: *mut c_void,
        index: i32,
        opened_locally: i32,
    ) -> Res<()> {
        unsafe { done((self.chan_init)(channel.cast(), conn, index, opened_locally)) }
    }
    unsafe fn add_open_channel(&self, conn: *mut c_void, channel: *mut UChannel) -> Res<()> {
        let array = unsafe {
            ptr::addr_of_mut!((*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).open_channels)
                .cast()
        };
        unsafe { done((self.arr_ptr_add)(array, channel.cast())) }
    }
    unsafe fn notify_accepting_channel(
        &self,
        conn: *mut c_void,
        channel: *mut UChannel,
    ) -> Res<bool> {
        let driver = unsafe {
            (*conn.cast::<hp2_abi::engine::generated::UNetConnection>()).driver
        };
        let mut accepted = 0;
        unsafe { done((self.driver_notify_accepting)(driver, channel.cast(), &mut accepted))?; }
        Ok(accepted != 0)
    }
    unsafe fn channel_received_nak(&self, channel: *mut UChannel, packet_id: i32) -> Res<()> {
        unsafe { done((self.chan_received_nak)(channel.cast(), packet_id)) }
    }
    unsafe fn channel_received_acks(&self, channel: *mut UChannel) -> Res<()> {
        unsafe { done((self.chan_received_acks)(channel.cast())) }
    }
    unsafe fn channel_received_raw(
        &self,
        channel: *mut UChannel,
        bunch: *mut FInBunch,
    ) -> Res<()> {
        unsafe { done((self.chan_received_raw)(channel.cast(), bunch.cast())) }
    }
    unsafe fn with_close_bunch<T>(
        &self,
        channel: *mut UChannel,
        body: impl FnOnce(*mut FOutBunch) -> Res<T>,
    ) -> Res<T> {
        unsafe { crate::net_bunch::with_out(&self.bunches, channel, 1, body) }
    }
    unsafe fn channel_send_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Res<()> {
        unsafe { done((self.chan_send_bunch)(channel.cast(), bunch.cast(), 0)) }
    }
    unsafe fn delete_channel(&self, channel: *mut UChannel) -> Res<()> {
        unsafe { done((self.chan_delete)(channel.cast())) }
    }
}

impl ConnectionTickOps for Hp2NetConnServices {
    fn app_round(&self, value: f32) -> i32 {
        unsafe { (self.app_round)(value) }
    }

    unsafe fn log_timeout(&self, timeout: f32, elapsed: f32) -> Res<()> {
        unsafe { done((self.conn_log_timeout)(timeout, elapsed)) }
    }

    unsafe fn actor_channel_exists(
        &self,
        connection: *mut UNetConnection,
        actor: *mut c_void,
    ) -> Res<bool> {
        Ok(!unsafe { (self.conn_actor_channel_find)(connection.cast(), actor) }.is_null())
    }

    unsafe fn set_bad_connection_alert(&self, actor: *mut c_void, value: bool) {
        unsafe { (self.actor_set_bad_connection_alert)(actor, i32::from(value)) }
    }

    unsafe fn channel_tick(&self, channel: *mut UChannel) -> Res<()> {
        unsafe { done((self.chan_tick)(channel.cast())) }
    }

    unsafe fn download_tick(&self, download: *mut c_void) -> Res<()> {
        unsafe { done((self.dl_tick)(download)) }
    }
}


impl ConnectionReceiveFileOps for Hp2NetConnServices {
    type Error = Thrown;

    unsafe fn check_fail(&self, expression: &std::ffi::CStr, line: i32) -> Res<()> {
        unsafe { ConnectionPacketOps::check_fail(self, expression, line) }
    }
    unsafe fn package_count(&self, conn: *mut UNetConnection) -> i32 {
        unsafe { (*(*conn).package_map.cast::<UPackageMap>()).list.num }
    }
    unsafe fn download_info_count(&self, conn: *mut UNetConnection) -> i32 {
        unsafe { (self.dl_info_num)(conn.cast()) }
    }
    unsafe fn add_default_download_info(&self, conn: *mut UNetConnection) -> Res<()> {
        unsafe { done((self.dl_info_add_default)(conn.cast())) }
    }
    unsafe fn delete_download(&self, download: *mut c_void) -> Res<()> {
        unsafe { done((self.dl_delete)(download)) }
    }
    unsafe fn construct_download(&self, conn: *mut UNetConnection) -> Res<*mut c_void> {
        let mut download = ptr::null_mut();
        unsafe { done((self.dl_construct)(conn.cast(), &mut download))?; }
        Ok(download)
    }
    unsafe fn receive_file(
        &self,
        conn: *mut UNetConnection,
        package_index: i32,
        params: *const u32,
        compression: i32,
    ) -> Res<()> {
        unsafe { done((self.dl_receive_file)(conn.cast(), package_index, params, compression)) }
    }
}

const _: () = assert!(size_of::<Hp2NetConnServices>() == 94 * size_of::<usize>());
const _: () = assert!(offset_of!(Hp2NetConnServices, conn_check_fail) == 0);
const _: () = assert!(offset_of!(Hp2NetConnServices, app_ceil_log_two) == size_of::<usize>());
const _: () = assert!(offset_of!(Hp2NetConnServices, log_close) == 2 * size_of::<usize>());
const _: () = assert!(offset_of!(Hp2NetConnServices, ar_log_int) == 3 * size_of::<usize>());
const _: () = assert!(offset_of!(Hp2NetConnServices, dl_tick) == 93 * size_of::<usize>());
