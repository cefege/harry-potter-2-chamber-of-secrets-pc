//! Rust implementation of `UNetConnection` behavior from `UnConn.cpp`.
//!
//! Source submodules are split by function cluster so parallel porting can keep
//! each control-flow unit isolated. C++ calls retained forwarding shells where
//! other engine translation units depend on the virtual/interface symbol.
use crate::net_sequence::make_relative;

use crate::net_bunch;
use crate::net_conn_abi::{
    ConnectionDestroyOps, ConnectionPacketOps, ConnectionReceiveOps, ConnectionReceiveFileOps,
    ConnectionTickOps, Hp2NetConnServices, Res, Thrown, done,
};
use hp2_abi::engine::generated::{
    DelayedPacket, FBitReader, FBitWriter, FDownloadInfo, FInBunch, FOutBunch, UChannel, UNetConnection,
    UNetDriver, UPackageMap,
};
use std::ffi::c_void;
use std::mem::MaybeUninit;

/// `UNetConnection::AssertValid` (`UnConn.cpp:134-144`).
///
/// # Safety
/// `conn` must point to a live `UNetConnection`; `svc` must point to the
/// connection services table. Invalid pointers retain the original raw crash.
pub unsafe fn assert_valid(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { assert_valid_with(conn, svc) }
}

/// Shared protocol/state validity checks for native and canonical connections.
pub unsafe fn assert_valid_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    ops: &O,
) -> Result<(), O::Error> {
    let conn = unsafe { &*conn };
    if conn.protocol_version < 1 {
        unsafe { ops.check_fail(c"ProtocolVersion>=MIN_PROTOCOL_VERSION", 139)?; }
    }
    if conn.protocol_version > 1 {
        unsafe { ops.check_fail(c"ProtocolVersion<=MAX_PROTOCOL_VERSION", 140)?; }
    }
    if conn.state != 1 && conn.state != 2 && conn.state != 3 {
        unsafe {
            ops.check_fail(
                c"State==USOCK_Closed || State==USOCK_Pending || State==USOCK_Open",
                141,
            )?;
        }
    }
    Ok(())
}

/// `UNetConnection::InitOut`; the service reconstructs `Out` as
/// `FBitWriter(MaxPacket*8)` so its C++ object lifetime remains intact.
///
/// # Safety
/// `conn` must point to a live connection and `svc` to its services table.
pub unsafe fn init_out(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { done((svc.conn_out_reset)(conn.cast())) }
}

/// `UNetConnection::Destroy` (`UnConn.cpp:72-113`).
///
/// # Safety
/// `conn` and every object reachable from it must have the same liveness
/// guarantees as the original C++ method. Raw member dereferences are
/// intentional; malformed engine pointers still crash.
pub unsafe fn destroy(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { destroy_with_ops(conn.cast(), &NativeConnectionDestroyOps { svc }) }
}

/// Shared `UNetConnection::Destroy` source order for native and Core-owned records.
///
/// # Safety
/// `conn` and the live objects reachable through the policy hooks must remain valid.
pub unsafe fn destroy_with_ops<O: ConnectionDestroyOps>(
    conn: *mut c_void,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        ops.log_close(conn)?;
        ops.close_channel_zero(conn)?;
        ops.flush_net(conn)?;
        ops.unregister(conn)?;
        ops.set_closed(conn);
        ops.delete_open_channels(conn)?;
        ops.delete_package_map(conn)?;
        ops.delete_download(conn)?;
        ops.super_destroy(conn)?;
    }
    Ok(())
}

struct NativeConnectionDestroyOps<'a> {
    svc: &'a Hp2NetConnServices,
}

impl ConnectionDestroyOps for NativeConnectionDestroyOps<'_> {
    type Error = Thrown;

    unsafe fn log_close(&self, conn: *mut c_void) -> Res<()> {
        unsafe { done((self.svc.log_close)(conn)) }
    }
    unsafe fn close_channel_zero(&self, conn: *mut c_void) -> Res<()> {
        let channel = unsafe { (*conn.cast::<UNetConnection>()).channels[0] };
        if channel.is_null() { return Ok(()); }
        unsafe { done((self.svc.chan_close)(channel)) }
    }
    unsafe fn flush_net(&self, conn: *mut c_void) -> Res<()> {
        unsafe { done((self.svc.conn_flush_net)(conn)) }
    }
    unsafe fn unregister(&self, conn: *mut c_void) -> Res<()> {
        unsafe {
            let connection = conn.cast::<UNetConnection>();
            let driver = (*connection).driver.cast::<UNetDriver>();
            let server = (*driver).server_connection;
            if !server.is_null() {
                if server != conn {
                    done((self.svc.conn_check_fail)(
                        c"Driver->ServerConnection==this".as_ptr(),
                        c"UnConn.cpp".as_ptr().cast::<c_void>(),
                        88,
                    ))?;
                }
                (*driver).server_connection = std::ptr::null_mut();
            } else {
                if !(*driver).server_connection.is_null() {
                    done((self.svc.conn_check_fail)(
                        c"Driver->ServerConnection==NULL".as_ptr(),
                        c"UnConn.cpp".as_ptr().cast::<c_void>(),
                        93,
                    ))?;
                }
                let mut removed = 0;
                done((self.svc.driver_remove_client)(driver.cast(), conn, &mut removed))?;
                if removed != 1 {
                    done((self.svc.conn_check_fail)(
                        c"Driver->ClientConnections.RemoveItem( this )==1".as_ptr(),
                        c"UnConn.cpp".as_ptr().cast::<c_void>(),
                        94,
                    ))?;
                }
            }
            Ok(())
        }
    }
    unsafe fn set_closed(&self, conn: *mut c_void) {
        unsafe { (*conn.cast::<UNetConnection>()).state = 1; }
    }
    unsafe fn delete_open_channels(&self, conn: *mut c_void) -> Res<()> {
        unsafe {
            let open = std::ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).open_channels)
                .cast::<c_void>();
            let mut index = (self.svc.arr_ptr_num)(open) - 1;
            while index >= 0 {
                let mut channel = std::ptr::null_mut();
                done((self.svc.arr_ptr_get)(open, index, &mut channel))?;
                done((self.svc.chan_delete)(channel))?;
                index -= 1;
            }
        }
        Ok(())
    }
    unsafe fn delete_package_map(&self, conn: *mut c_void) -> Res<()> {
        let package_map = unsafe { (*conn.cast::<UNetConnection>()).package_map };
        unsafe { done((self.svc.pkg_delete)(package_map)) }
    }
    unsafe fn delete_download(&self, conn: *mut c_void) -> Res<()> {
        let download = unsafe { (*conn.cast::<UNetConnection>()).download };
        if download.is_null() { return Ok(()); }
        unsafe { done((self.svc.dl_delete)(download)) }
    }
    unsafe fn super_destroy(&self, conn: *mut c_void) -> Res<()> {
        unsafe { done((self.svc.conn_super_destroy)(conn)) }
    }
}

/// `UNetConnection::Serialize(FArchive&)` (`UnConn.cpp:60-71`).
///
/// # Safety
/// `conn`, `archive`, and `svc` must remain valid for the call; the archive's
/// operations retain the original C++ object lifetime and throw behavior.
pub unsafe fn serialize(
    conn: *mut UNetConnection,
    archive: *mut c_void,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe {
        done((svc.conn_super_serialize)(conn.cast(), archive))?;
        done((svc.ar_serialize_package_map)(archive, conn.cast()))?;
        for index in 0..1023 {
            done((svc.ar_serialize_channel)(archive, conn.cast(), index))?;
        }
        done((svc.ar_serialize_download)(archive, conn.cast()))?;
    }
    Ok(())
}

/// `UNetConnection::Exec` (`UnConn.cpp:114-133`).
///
/// # Safety
/// `conn`, `cmd`, `ar`, and `svc` must point to live engine objects.
pub unsafe fn exec(
    conn: *mut UNetConnection,
    cmd: *const c_void,
    ar: *mut c_void,
    svc: &Hp2NetConnServices,
) -> Res<bool> {
    unsafe {
        let mut cursor = cmd;
        if (svc.conn_parse_command)(&mut cursor, 0) != 0 {
            done((svc.ar_log_int)(
                ar,
                (svc.app_round)((*conn).best_lag * 1000.0),
            ))?;
            return Ok(true);
        }
        if (svc.conn_parse_command)(&mut cursor, 1) != 0 {
            done((svc.ar_log_int)(ar, (svc.app_round)((*conn).in_loss)))?;
            return Ok(true);
        }
        let mut handled = 0;
        done((svc.conn_super_exec)(conn.cast(), cursor, ar, &mut handled))?;
        Ok(handled != 0)
    }
}

/// Operations for the shared `UNetConnection::SendPackageMap` policy.
pub(crate) trait SendPackageMapOps {
    type Error;

    unsafe fn package_count(&self, connection: *mut UNetConnection) -> std::result::Result<i32, Self::Error>;
    unsafe fn log_package_use(&self, connection: *mut UNetConnection, index: i32) -> std::result::Result<(), Self::Error>;
    unsafe fn download_manager_count(&self, driver: *mut UNetDriver) -> std::result::Result<i32, Self::Error>;
    unsafe fn load_download_class(&self, driver: *mut UNetDriver, index: i32) -> std::result::Result<*mut c_void, Self::Error>;
    unsafe fn log_download_defaults(&self, connection: *mut UNetConnection, class: *mut c_void) -> std::result::Result<(), Self::Error>;
}

/// Shared `UNetConnection::SendPackageMap` source order for native and Core.
///
/// # Safety
/// `conn`, its driver and package map are live records for the supplied operations.
pub(crate) unsafe fn send_package_map_with<O: SendPackageMapOps>(
    conn: *mut UNetConnection,
    ops: &O,
) -> std::result::Result<(), O::Error> {
    // SAFETY: caller guarantees the connection and its related records are live.
    unsafe {
        let mut index = 0;
        while index < ops.package_count(conn)? {
            ops.log_package_use(conn, index)?;
            index += 1;
        }
        let driver = (*conn).driver.cast::<UNetDriver>();
        index = 0;
        while index < ops.download_manager_count(driver)? {
            let class = ops.load_download_class(driver, index)?;
            if !class.is_null() {
                ops.log_download_defaults(conn, class)?;
            }
            index += 1;
        }
    }
    Ok(())
}

struct NativeSendPackageMapOps<'a>(&'a Hp2NetConnServices);

impl SendPackageMapOps for NativeSendPackageMapOps<'_> {
    type Error = Thrown;

    unsafe fn package_count(&self, conn: *mut UNetConnection) -> Res<i32> {
        Ok(unsafe { (*(*conn).package_map.cast::<UPackageMap>()).list.num })
    }
    unsafe fn log_package_use(&self, conn: *mut UNetConnection, index: i32) -> Res<()> {
        unsafe { done((self.0.pkg_log_uses)(conn.cast(), index)) }
    }
    unsafe fn download_manager_count(&self, driver: *mut UNetDriver) -> Res<i32> {
        Ok(unsafe { (*driver).download_managers.num })
    }
    unsafe fn load_download_class(&self, driver: *mut UNetDriver, index: i32) -> Res<*mut c_void> {
        let mut class = std::ptr::null_mut();
        unsafe { done((self.0.dlmgr_load_class)(driver.cast(), index, &mut class))?; }
        Ok(class)
    }
    unsafe fn log_download_defaults(&self, conn: *mut UNetConnection, class: *mut c_void) -> Res<()> {
        unsafe { done((self.0.dlmgr_log_defaults)(conn.cast(), class)) }
    }
}

/// `UNetConnection::SendPackageMap` (`UnConn.cpp:145-193`).
///
/// # Safety
/// `conn` and its driver/package map must be live engine objects.
pub unsafe fn send_package_map(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { send_package_map_with(conn, &NativeSendPackageMapOps(svc)) }
}

/// `UNetConnection::PurgeAcks` (`UnConn.cpp:626-633`).
///
/// # Safety
/// `conn` must point to a live connection.
pub unsafe fn purge_acks(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { purge_acks_with(conn, svc) }
}

/// Shared `PurgeAcks` policy used by both native forwarding and Core-owned records.
///
/// # Safety
/// `conn` and its acknowledgement arrays must be live and initialized.
pub unsafe fn purge_acks_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    ops: &O,
) -> Result<(), O::Error> {
    let count = unsafe { ops.resend_ack_count(conn.cast()) };
    for index in 0..count {
        let ack = unsafe { ops.resend_ack(conn.cast(), index)? };
        unsafe { send_ack_with(conn, ack, 0, ops)?; }
    }
    unsafe { ops.empty_resend_acks(conn.cast(), 32) }
}

/// `UNetConnection::ReceivedNak` (`UnConn.cpp:344-357`).
///
/// # Safety
/// `conn` and its open-channel entries must be live engine objects.
pub unsafe fn received_nak(
    conn: *mut UNetConnection,
    nak_packet_id: i32,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe { received_nak_with(conn, nak_packet_id, svc) }
}

/// Source-ordered `ReceivedNak` body shared by native and Core connections.
///
/// # Safety
/// `conn` and its open-channel records must be live and initialized.
pub unsafe fn received_nak_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    nak_packet_id: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        let count = ops.open_channel_count(conn.cast());
        for index in (0..count).rev() {
            let channel = ops.open_channel_at(conn.cast(), index)?;
            ops.channel_received_nak(channel, nak_packet_id)?;
            if (*channel).open_packet_id == nak_packet_id {
                ops.channel_received_acks(channel)?;
            }
        }
    }
    Ok(())
}

/// `UNetConnection::HandleClientPlayer` (`UnConn.cpp:901-928`).
///
/// # Safety
/// `conn`, `pawn`, the client/viewport graph, and `svc` must be live engine
/// objects. A null object reached through a raw C++ dereference remains a crash.
pub unsafe fn handle_client_player(
    conn: *mut UNetConnection,
    pawn: *mut c_void,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe {
        crate::connection_player_owner::handle_native_client_player(conn, pawn, svc)
            .map_err(|_| Thrown)
    }
}

/// `UNetConnection::ReceiveFile` (`UnConn.cpp:938-955`).
///
/// # Safety
/// `conn` and its package map/download-info objects must be live engine objects.
pub unsafe fn receive_file(
    conn: *mut UNetConnection,
    package_index: i32,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe { receive_file_with(conn, package_index, svc) }
}

/// Shared native/Core `ReceiveFile` ordering and validation.
///
/// # Safety
/// `conn` and its package map/download-info objects must be live engine objects.
pub unsafe fn receive_file_with<O: crate::net_conn_abi::ConnectionReceiveFileOps>(
    conn: *mut UNetConnection,
    package_index: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        if package_index < 0 || package_index >= ops.package_count(conn) {
            ops.check_fail(c"PackageMap->List.IsValidIndex(PackageIndex)", 941)?;
        }
        if ops.download_info_count(conn) == 0 {
            ops.add_default_download_info(conn)?;
        }
        let old_download = (*conn).download;
        if !old_download.is_null() {
            ops.delete_download(old_download)?;
            (*conn).download = std::ptr::null_mut();
        }
        let download = ops.construct_download(conn)?;
        (*conn).download = download;
        let info = (*conn).download_info.data.cast::<FDownloadInfo>();
        let params = (*info).params.data.cast::<u32>();
        let compression = (*info).compression;
        ops.receive_file(conn, package_index, params, compression)?;
    }
    Ok(())
}

const MAX_PACKET_ID: i32 = 16_384;
const MAX_PACKET_HEADER_BITS: i32 = 16;
const MAX_PACKET_TRAILER_BITS: i32 = 1;

/// `UNetConnection::PreSend` (`UnConn.cpp:586-606`), shared across packet Ops.
///
/// # Safety
/// `conn` points to a live connection with initialized packet storage.
pub unsafe fn pre_send(
    conn: *mut UNetConnection,
    size_bits: i32,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe { pre_send_with(conn, size_bits, svc) }
}

/// Generic `PreSend` implementation used by native and canonical connection owners.
///
/// # Safety
/// `conn` points to a live connection with initialized packet storage.
pub unsafe fn pre_send_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    size_bits: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        let writer = ops.out(conn.cast());
        if ops.writer_num_bits(writer) + size_bits + MAX_PACKET_TRAILER_BITS > (*conn).max_packet * 8 {
            ops.flush_net(conn.cast())?;
        }
        if ops.writer_num_bits(writer) == 0 {
            ops.writer_write_int(writer, (*conn).out_packet_id, MAX_PACKET_ID)?;
            if ops.writer_num_bits(writer) > MAX_PACKET_HEADER_BITS {
                ops.check_fail(c"Out.GetNumBits()<=MAX_PACKET_HEADER_BITS", 598)?;
            }
        }
        let num_bits = ops.writer_num_bits(writer);
        let max_bits = (*conn).max_packet * 8;
        if num_bits + size_bits + MAX_PACKET_TRAILER_BITS > max_bits {
            ops.pre_send_overflow(num_bits, size_bits, max_bits)?;
        }
    }
    Ok(())
}

/// `UNetConnection::PostSend` (`UnConn.cpp:611-621`).
///
/// # Safety
/// `conn` points to a live connection with initialized packet storage.
pub unsafe fn post_send(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { post_send_with(conn, svc) }
}

/// Generic `PostSend` implementation used by native and canonical connection owners.
///
/// # Safety
/// `conn` points to a live connection with initialized packet storage.
pub unsafe fn post_send_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        let writer = ops.out(conn.cast());
        let num_bits = ops.writer_num_bits(writer);
        let max_bits = (*conn).max_packet * 8;
        if num_bits > max_bits {
            ops.check_fail(c"Out.GetNumBits()<=MaxPacket*8", 616)?;
        }
        if num_bits == max_bits {
            ops.flush_net(conn.cast())?;
        }
    }
    Ok(())
}

/// `UNetConnection::SendAck` (`UnConn.cpp:638-655`).
///
/// # Safety
/// `conn` must point to a live connection.
pub unsafe fn send_ack(
    conn: *mut UNetConnection,
    ack_packet_id: i32,
    first_time: i32,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe { send_ack_with(conn, ack_packet_id, first_time, svc) }
}

/// Shared `SendAck` policy used by both native forwarding and Core-owned records.
///
/// # Safety
/// `conn` and its packet writer/acknowledgement arrays must be live and initialized.
pub unsafe fn send_ack_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    ack_packet_id: i32,
    first_time: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        if (*conn).internal_ack == 0 {
            if first_time != 0 {
                purge_acks_with(conn, ops)?;
                ops.add_queued_ack(conn.cast(), ack_packet_id)?;
            }
            pre_send_with(conn, ops.ceil_log_two(MAX_PACKET_ID as u32) + 1, ops)?;
            let writer = ops.out(conn.cast());
            ops.writer_write_bit(writer, 1)?;
            ops.writer_write_int(writer, ack_packet_id, MAX_PACKET_ID)?;
            (*conn).allow_merge = 0;
            post_send_with(conn, ops)?;
        }
    }
    Ok(())
}

/// `FTime` `FIXTIME` (`Core/Inc/Core.h:28-66`).
const FIXTIME: f32 = 4_294_967_296.0;

/// `FTime::GetFloat()`: `v / FIXTIME` in float.
fn ftime_get_float(v: i64) -> f32 {
    (v as f32) / FIXTIME
}

/// `FTime::operator+(float)`: `v + (TIMETYP)(f * FIXTIME)`. Rust float-to-int
/// casts saturate where C++ leaves out-of-range values undefined.
fn ftime_add_f32(v: i64, f: f32) -> i64 {
    v.wrapping_add((f * FIXTIME) as i64)
}

/// `FTime::operator-(FTime)`: `(v - t.v) / FIXTIME` in float.
fn ftime_sub(a: i64, b: i64) -> f32 {
    (a.wrapping_sub(b) as f32) / FIXTIME
}

// `MakeRelative` is shared with `Engine/Inc/UnNet.h` through
// `crate::net_sequence`; retain one Rust owner for all protocol sequence math.

/// Engine object reached through the connection's `Driver` member.
unsafe fn driver_of(conn: *mut UNetConnection) -> *mut UNetDriver {
    unsafe { (*conn).driver.cast::<UNetDriver>() }
}

/// `UNetConnection::ReceivedRawPacket` (`UnConn.cpp:203-231`).
///
/// `debugfSlow` and `appErrorfSlow` expand to `GNull->Logf` in the product
/// build (`Core/Inc/UnFile.h:69-75`, `DO_GUARD_SLOW` 0), so those logs are dropped.
///
/// # Safety
/// `conn` must point to a live connection; `data` must point to at least
/// `count` readable bytes when `count > 0`.
pub unsafe fn received_raw_packet(
    conn: *mut UNetConnection,
    data: *const u8,
    count: i32,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe { received_raw_packet_with(conn, data, count, svc) }
}

/// Shared raw receive entry. The reader's buffer and archive identity are
/// released on every path after successful construction.
///
/// # Safety
/// `conn` must point to a live connection; `data` must point to `count`
/// readable bytes when `count > 0`.
pub unsafe fn received_raw_packet_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    data: *const u8,
    count: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        (*conn).in_byte_acc = (*conn)
            .in_byte_acc
            .wrapping_add(count.wrapping_add((*conn).packet_overhead));
        (*conn).in_pkt_acc = (*conn).in_pkt_acc.wrapping_add(1);
        if count > 0 {
            let mut last_byte = *data.add((count - 1) as usize);
            if last_byte != 0 {
                let mut bit_size = count.wrapping_mul(8).wrapping_sub(1);
                while last_byte & 0x80 == 0 {
                    last_byte = last_byte.wrapping_mul(2);
                    bit_size = bit_size.wrapping_sub(1);
                }
                let mut reader_storage = MaybeUninit::<FBitReader>::uninit();
                let reader = reader_storage.as_mut_ptr();
                ops.reader_construct(reader)?;
                let result = ops
                    .reader_initialize(reader, data, bit_size)
                    .and_then(|()| received_packet_with(conn, reader, ops));
                let release = ops.reader_release(reader);
                let destroy = ops.reader_destruct(reader);
                result.and(release).and(destroy)?;
            }
        }
    }
    Ok(())
}

/// Native `FlushNet` adapter retained for C++ callers until cutover.
///
/// # Safety
/// `conn` points to a live connection and `svc` to a live native adapter.
pub unsafe fn flush_net(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { flush_net_with(conn, svc) }
}

/// Shared `UNetConnection::FlushNet` packet policy.
///
/// Native and canonical owners provide packet primitives through
/// [`ConnectionPacketOps`]; packet ordering, loss, lag, duplicate, counters,
/// keepalive and queued-ack semantics remain one implementation.
///
/// # Safety
/// `conn` points to a live connection with initialized Out and driver state.
pub unsafe fn flush_net_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        let out = ops.out(conn.cast());
        if ops.writer_is_error(out) {
            ops.check_fail(c"!Out.IsError()", 237)?;
        }
        ops.reset_last_end(conn.cast());
        (*conn).time_sensitive = 0;
        let driver = driver_of(conn);
        if ops.writer_num_bits(out) != 0
            || ftime_sub(
                i64::from_ne_bytes((*driver).time),
                i64::from_ne_bytes((*conn).last_send_time),
            ) > (*driver).keep_alive_time
        {
            if ops.writer_num_bits(out) == 0 {
                pre_send_with(conn, 0, ops)?;
            }
            ops.writer_write_bit(out, 1)?;
            if ops.writer_is_error(out) {
                ops.check_fail(c"!Out.IsError()", 250)?;
            }
            while ops.writer_num_bits(out) & 7 != 0 {
                ops.writer_write_bit(out, 0)?;
            }
            if ops.writer_is_error(out) {
                ops.check_fail(c"!Out.IsError()", 253)?;
            }

            if (*conn).pkt_order != 0 {
                push_delayed_copy_with(conn, out, ops)?;
                let mut i = (*conn).delayed.num - 1;
                while i >= 0 {
                    if ops.random() as f64 > 0.50 {
                        if (*conn).pkt_loss == 0
                            || ops.random() * 100.0f32 > (*conn).pkt_loss as f32
                        {
                            let elem = ops.delayed_at(conn.cast(), i)?;
                            let data = ops.delayed_data(elem)?;
                            ops.low_level_send(
                                conn.cast(),
                                data,
                                ops.delayed_data_num(elem),
                            )?;
                        }
                        ops.delayed_remove(conn.cast(), i)?;
                    }
                    i -= 1;
                }
            } else if (*conn).pkt_lag != 0 {
                if (*conn).pkt_loss == 0 || ops.random() * 100.0f32 > (*conn).pkt_loss as f32 {
                    let elem = push_delayed_copy_with(conn, out, ops)?;
                    (*elem.cast::<DelayedPacket>()).send_time =
                        ftime_add_f32(ops.seconds(), (*conn).pkt_lag as f32 / 1000.0f32)
                            .to_ne_bytes();
                }
            } else if (*conn).pkt_loss == 0
                || ops.random() * 100.0f32 >= (*conn).pkt_loss as f32
            {
                ops.low_level_send(
                    conn.cast(),
                    ops.writer_data(out),
                    ops.writer_num_bytes(out),
                )?;
                if (*conn).pkt_dup != 0 && ops.random() * 100.0f32 < (*conn).pkt_dup as f32 {
                    ops.low_level_send(
                        conn.cast(),
                        ops.writer_data(out),
                        ops.writer_num_bytes(out),
                    )?;
                }
            }

            let index = ((*conn).out_packet_id & 255) as usize;
            (*conn).out_lag_packet_id[index] = (*conn).out_packet_id;
            (*conn).out_lag_time[index] = (*driver).time;
            (*conn).out_packet_id = (*conn).out_packet_id.wrapping_add(1);
            (*conn).out_pkt_acc = (*conn).out_pkt_acc.wrapping_add(1);
            (*conn).last_send_time = (*driver).time;
            let bytes = ops.writer_num_bytes(out).wrapping_add((*conn).packet_overhead);
            (*conn).queued_bytes = (*conn).queued_bytes.wrapping_add(bytes);
            (*conn).out_byte_acc = (*conn).out_byte_acc.wrapping_add(bytes);
            ops.init_out(conn.cast())?;
        }

        let mut i = 0;
        while i < ops.queued_ack_count(conn.cast()) {
            let ack = ops.queued_ack(conn.cast(), i)?;
            ops.add_resend_ack(conn.cast(), ack)?;
            i += 1;
        }
        ops.empty_queued_acks(conn.cast(), 32)?;
    }
    Ok(())
}

/// Copies `Out` into a new delayed packet using the selected owner Ops.
unsafe fn push_delayed_copy_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    out: *mut c_void,
    ops: &O,
) -> Result<*mut c_void, O::Error> {
    unsafe {
        let elem = ops.delayed_add(conn.cast())?;
        let bytes = ops.writer_num_bytes(out);
        ops.delayed_data_add(elem, bytes)?;
        let dst = ops.delayed_data(elem)?;
        std::ptr::copy_nonoverlapping(
            ops.writer_data(out).cast::<u8>(),
            dst.cast::<u8>(),
            bytes as usize,
        );
        Ok(elem)
    }
}


/// `UNetConnection::Serialize(const TCHAR*, EName)` (`UnConn.cpp:312-321`).
///
/// # Safety
/// `conn` must be live; `data` is the message payload forwarded to the control channel.
pub unsafe fn serialize_text(
    conn: *mut UNetConnection,
    data: *const c_void,
    msg_type: i32,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe {
        let control = (*conn).channels[0].cast::<UChannel>();
        if !control.is_null() && (*control).closing == 0 {
            done((svc.control_chan_serialize)(control.cast(), data, msg_type))?;
        }
    }
    Ok(())
}

/// `UNetConnection::IsNetReady` (`UnConn.cpp:322-332`) base policy shared by
/// the native and canonical connection owners. Virtual subclasses apply their
/// own override before calling this operation.
///
/// # Safety
/// `conn` is live and `ops` provides the corresponding output writer.
pub(crate) unsafe fn is_net_ready_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    saturate: i32,
    ops: &O,
) -> i32 {
    unsafe {
        let out = ops.out(conn.cast());
        if saturate != 0 {
            (*conn).queued_bytes = ops.writer_num_bytes(out).wrapping_neg();
        }
        ((*conn).queued_bytes.wrapping_add(ops.writer_num_bytes(out)) <= 0) as i32
    }
}

/// Native service adapter for the shared `UNetConnection::IsNetReady` base policy.
///
/// # Safety
/// `conn` must point to a live connection.
pub unsafe fn is_net_ready(
    conn: *mut UNetConnection,
    saturate: i32,
    svc: &Hp2NetConnServices,
) -> i32 {
    unsafe { is_net_ready_with(conn, saturate, svc) }
}

/// `UNetConnection::ReadInput` (`UnConn.cpp:333-334`): the C++ body is empty.
pub fn read_input(_delta_seconds: f32) {}

/// `MAX_CHANNELS` (channel index limit, exclusive).
const MAX_CHANNELS: i32 = 1023;
/// `MAX_CHSEQUENCE` (reliable sequence limit, exclusive).
const MAX_CHSEQUENCE: i32 = 1024;
/// `CHTYPE_MAX` (number of channel types, exclusive).
const CHTYPE_MAX: i32 = 8;
/// `CHTYPE_Control`.
const CHTYPE_CONTROL: i32 = 1;
/// `MAX_BUNCH_HEADER_BITS` (`FBitWriter` capacity for bunch headers).
const MAX_BUNCH_HEADER_BITS: i32 = 64;
/// `USOCK_Closed` (`UnConn.h:26`).
const USOCK_CLOSED: i32 = 1;

/// `UNetConnection::ReceivedPacket(FBitReader&)` (`UnConn.cpp:362-577`).
pub unsafe fn received_packet(
    conn: *mut UNetConnection,
    reader: *mut c_void,
    svc: &Hp2NetConnServices,
) -> Res<()> {
    unsafe { received_packet_with(conn, reader.cast(), svc) }
}

/// Shared packet ordering, acknowledgement, and bunch dispatch.
pub unsafe fn received_packet_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    reader: *mut FBitReader,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        assert_valid_with(conn, ops)?;
        if ops.reader_is_error(reader) {
            return Ok(());
        }
        (*conn).last_receive_time = (*driver_of(conn)).time;
        let packet_id = make_relative(
            ops.reader_read_int(reader, MAX_PACKET_ID),
            (*conn).in_packet_id,
            MAX_PACKET_ID,
        );
        if packet_id > (*conn).in_packet_id {
            (*conn).in_loss_acc = (*conn)
                .in_loss_acc
                .wrapping_add(packet_id.wrapping_sub((*conn).in_packet_id).wrapping_sub(1));
            (*conn).in_packet_id = packet_id;
        } else {
            (*conn).in_ord_acc = (*conn).in_ord_acc.wrapping_add(1);
        }
        send_ack_with(conn, packet_id, 1, ops)?;
        while !ops.reader_at_end(reader) && (*conn).state != USOCK_CLOSED {
            let is_ack = ops.reader_read_bit(reader);
            if ops.reader_is_error(reader) {
                return Ok(());
            }
            let proceed = if is_ack != 0 {
                received_ack_packet_with(conn, reader, ops)?
            } else {
                received_bunch_with(conn, reader, packet_id, ops)?
            };
            if !proceed {
                return Ok(());
            }
        }
    }
    Ok(())
}

unsafe fn received_ack_packet_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    reader: *mut FBitReader,
    ops: &O,
) -> Result<bool, O::Error> {
    unsafe {
        let ack_packet_id = make_relative(
            ops.reader_read_int(reader, MAX_PACKET_ID),
            (*conn).out_ack_packet_id,
            MAX_PACKET_ID,
        );
        if ops.reader_is_error(reader) {
            return Ok(false);
        }
        if ack_packet_id > (*conn).out_ack_packet_id {
            let mut nak_packet_id = (*conn).out_ack_packet_id.wrapping_add(1);
            while nak_packet_id < ack_packet_id {
                received_nak_with(conn, nak_packet_id, ops)?;
                nak_packet_id = nak_packet_id.wrapping_add(1);
                (*conn).out_loss_acc = (*conn).out_loss_acc.wrapping_add(1);
            }
            (*conn).out_ack_packet_id = ack_packet_id;
        }
        let index = (ack_packet_id & 255) as usize;
        if (*conn).out_lag_packet_id[index] == ack_packet_id {
            let lag = ftime_sub(
                i64::from_ne_bytes((*driver_of(conn)).time),
                i64::from_ne_bytes((*conn).out_lag_time[index]),
            ) - (*conn).frame_time * 0.5f32;
            (*conn).lag_acc += lag;
            (*conn).lag_count = (*conn).lag_count.wrapping_add(1);
        }
        let count = ops.open_channel_count(conn.cast());
        for index in (0..count).rev() {
            let channel = ops.open_channel_at(conn.cast(), index)?;
            let mut out = (*channel).out_rec.cast::<FOutBunch>();
            while !out.is_null() {
                if (*out).packet_id == ack_packet_id {
                    (*out).received_ack = 1;
                    if (*out).b_open != 0 {
                        (*channel).open_acked = 1;
                    }
                }
                out = (*out).next.cast();
            }
            if (*channel).open_packet_id == ack_packet_id {
                (*channel).open_acked = 1;
            }
            ops.channel_received_acks(channel)?;
        }
    }
    Ok(true)
}

unsafe fn received_bunch_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    reader: *mut FBitReader,
    packet_id: i32,
    ops: &O,
) -> Result<bool, O::Error> {
    unsafe {
        let mut storage = MaybeUninit::<FInBunch>::uninit();
        let bunch = storage.as_mut_ptr();
        ops.bunch_construct(bunch)?;
        net_bunch::initialize_in(bunch, conn);
        let result = received_bunch_body_with(conn, reader, packet_id, bunch, ops);
        let release = ops.bunch_release(bunch);
        let destroy = ops.bunch_destruct(bunch);
        result.and_then(|value| release.and(destroy).map(|()| value))
    }
}

unsafe fn received_bunch_body_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    reader: *mut FBitReader,
    packet_id: i32,
    bunch: *mut FInBunch,
    ops: &O,
) -> Result<bool, O::Error> {
    unsafe {
        let b_control = ops.reader_read_bit(reader) as u8;
        (*bunch).packet_id = packet_id;
        (*bunch).b_open = if b_control != 0 { ops.reader_read_bit(reader) as u8 } else { 0 };
        (*bunch).b_close = if b_control != 0 { ops.reader_read_bit(reader) as u8 } else { 0 };
        (*bunch).b_reliable = ops.reader_read_bit(reader) as u8;
        (*bunch).ch_index = ops.reader_read_int(reader, MAX_CHANNELS);
        (*bunch).ch_sequence = if (*bunch).b_reliable != 0 {
            make_relative(
                ops.reader_read_int(reader, MAX_CHSEQUENCE),
                (*conn).in_reliable[(*bunch).ch_index as usize],
                MAX_CHSEQUENCE,
            )
        } else {
            0
        };
        (*bunch).ch_type = if (*bunch).b_reliable != 0 || (*bunch).b_open != 0 {
            ops.reader_read_int(reader, CHTYPE_MAX)
        } else {
            0
        };
        let bunch_data_bits = ops.reader_read_int(reader, (*conn).max_packet.wrapping_mul(8));
        if ops.reader_is_error(reader) {
            return Ok(false);
        }
        set_bunch_data_with(bunch, reader, bunch_data_bits, ops)?;
        if ops.reader_is_error(reader) {
            return Ok(false);
        }
        let ch_index = (*bunch).ch_index as usize;
        let ch_type = (*bunch).ch_type;
        if (*conn).channels[ch_index].is_null()
            && (*conn).channels[0].is_null()
            && (ch_index != 0 || ch_type != CHTYPE_CONTROL)
        {
            return Ok(false);
        }
        let mut channel = (*conn).channels[ch_index].cast::<UChannel>();
        if (*bunch).b_reliable != 0 && (*bunch).ch_sequence <= (*conn).in_reliable[ch_index] {
            return Ok(true);
        }
        if (*bunch).b_reliable == 0
            && ((*bunch).b_open == 0 || (*bunch).b_close == 0)
            && (channel.is_null() || (*channel).open_packet_id == -1)
        {
            return Ok(true);
        }
        if channel.is_null() {
            if !ops.known_channel_type(ch_type)? {
                return Ok(false);
            }
            channel = create_channel_with(conn, ch_type, 0, (*bunch).ch_index, ops)?;
            let accepted = ops.notify_accepting_channel(conn.cast(), channel)?;
            if !accepted {
                refuse_channel_with(conn, channel, (*bunch).ch_index, ops)?;
                return Ok(true);
            }
        }
        if (*bunch).b_open != 0 {
            (*channel).open_acked = 1;
            (*channel).open_packet_id = packet_id;
        }
        ops.channel_received_raw(channel, bunch)?;
        (*conn).in_bun_acc = (*conn).in_bun_acc.wrapping_add(1);
        Ok(true)
    }
}

unsafe fn set_bunch_data_with<O: ConnectionReceiveOps>(
    bunch: *mut FInBunch,
    reader: *mut FBitReader,
    count_bits: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        set_bunch_data_fields(bunch.cast(), reader, count_bits, |bytes| {
            ops.bunch_resize_buffer(bunch, bytes)
        })
    }
}

unsafe fn set_bunch_data_fields<E>(
    bunch: *mut FBitReader,
    reader: *mut FBitReader,
    count_bits: i32,
    resize: impl FnOnce(i32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        (*bunch).num = count_bits;
        (*bunch).pos = 0;
        (*bunch).ar_is_error = 0;
        resize(count_bits.wrapping_add(7) >> 3)?;
        hp2_core::bits::reader_serialize_bits(reader, (*bunch).buffer.data.cast(), count_bits);
    }
    Ok(())
}

#[cfg(test)]
unsafe fn set_bunch_data(
    bunch: *mut FBitReader,
    reader: *mut FBitReader,
    count_bits: i32,
) -> Res<()> {
    unsafe {
        set_bunch_data_fields(bunch, reader, count_bits, |bytes| {
            net_bunch::resize_buffer(&raw mut (*bunch).buffer, bytes)
        })
    }
}

unsafe fn refuse_channel_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    channel: *mut UChannel,
    ch_index: i32,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        ops.with_close_bunch(channel, |close| {
            if ops.out_bunch_is_error(close) {
                ops.check_fail(c"!CloseBunch.IsError()", 545)?;
            }
            if (*close).b_close == 0 {
                ops.check_fail(c"CloseBunch.bClose", 546)?;
            }
            (*close).b_reliable = 1;
            ops.channel_send_bunch(channel, close)?;
            flush_net_with(conn, ops)?;
            ops.delete_channel(channel)?;
            if ch_index == 0 {
                (*conn).state = USOCK_CLOSED;
            }
            Ok(())
        })
    }
}

/// Native `SendRawBunch` adapter retained for C++ callers until cutover.
///
/// # Safety
/// `conn` and `bunch` are live connection and outgoing-bunch records.
pub unsafe fn send_raw_bunch(
    conn: *mut UNetConnection,
    bunch: *mut FOutBunch,
    allow_merge: i32,
    svc: &Hp2NetConnServices,
) -> Res<i32> {
    unsafe { send_raw_bunch_with(conn, bunch, allow_merge, svc) }
}

/// Shared native `SendRawBunch` header and packet policy.
///
/// # Safety
/// `conn` and `bunch` are live records; the connection's Out writer is initialized.
pub unsafe fn send_raw_bunch_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    bunch: *mut FOutBunch,
    allow_merge: i32,
    ops: &O,
) -> Result<i32, O::Error> {
    unsafe {
        if (*bunch).received_ack != 0 {
            ops.check_fail(c"!Bunch.ReceivedAck", 663)?;
        }
        if ops.out_bunch_is_error(bunch) {
            ops.check_fail(c"!Bunch.IsError()", 664)?;
        }
        (*conn).out_bun_acc = (*conn).out_bun_acc.wrapping_add(1);
        (*conn).time_sensitive = 1;

        let mut header_storage = MaybeUninit::<FBitWriter>::uninit();
        let header = header_storage.as_mut_ptr();
        ops.writer_construct(header)?;
        let result = ops
            .writer_initialize(header, MAX_BUNCH_HEADER_BITS)
            .and_then(|()| send_header_and_bunch_with(conn, bunch, allow_merge, header.cast(), ops));
        let release = ops.writer_release_buffer(header);
        let destroy = ops.writer_destruct(header);
        result.and_then(|value| release.and(destroy).map(|()| value))
    }
}

/// Header construction, pre-send, and packet copy shared by all connection owners.
unsafe fn send_header_and_bunch_with<O: ConnectionPacketOps>(
    conn: *mut UNetConnection,
    bunch: *mut FOutBunch,
    allow_merge: i32,
    header: *mut c_void,
    ops: &O,
) -> Result<i32, O::Error> {
    unsafe {
        let bunch_ptr = bunch.cast::<c_void>();
        ops.writer_write_bit(header, 0)?;
        ops.writer_write_bit(header, ((*bunch).b_open != 0 || (*bunch).b_close != 0) as i32)?;
        if (*bunch).b_open != 0 || (*bunch).b_close != 0 {
            ops.writer_write_bit(header, (*bunch).b_open as i32)?;
            ops.writer_write_bit(header, (*bunch).b_close as i32)?;
        }
        ops.writer_write_bit(header, (*bunch).b_reliable as i32)?;
        ops.writer_write_int(header, (*bunch).ch_index, MAX_CHANNELS)?;
        if (*bunch).b_reliable != 0 {
            ops.writer_write_int(header, (*bunch).ch_sequence, MAX_CHSEQUENCE)?;
        }
        if (*bunch).b_reliable != 0 || (*bunch).b_open != 0 {
            ops.writer_write_int(header, (*bunch).ch_type, CHTYPE_MAX)?;
        }
        ops.writer_write_int(
            header,
            ops.writer_num_bits(bunch_ptr),
            (*conn).max_packet.wrapping_mul(8),
        )?;
        if ops.writer_is_error(header) {
            ops.check_fail(c"!Header.IsError()", 684)?;
        }

        pre_send_with(
            conn,
            ops.writer_num_bits(header).wrapping_add(ops.writer_num_bits(bunch_ptr)),
            ops,
        )?;

        (*conn).allow_merge = allow_merge;
        (*bunch).packet_id = (*conn).out_packet_id;
        (*bunch).time = (*driver_of(conn)).time;
        ops.mark_last_start(conn.cast());
        let out = ops.out(conn.cast());
        ops.writer_serialize_bits(out, ops.writer_data(header), ops.writer_num_bits(header))?;
        ops.writer_serialize_bits(out, ops.writer_data(bunch_ptr), ops.writer_num_bits(bunch_ptr))?;
        post_send_with(conn, ops)?;
        Ok((*bunch).packet_id)
    }
}

/// `UNetConnection::CreateChannel` (`UnConn.cpp:713-745`). Returns null when
/// no free channel index exists.
pub unsafe fn create_channel(
    conn: *mut UNetConnection,
    ch_type: i32,
    b_opened_locally: i32,
    ch_index: i32,
    svc: &Hp2NetConnServices,
) -> Res<*mut c_void> {
    unsafe {
        create_channel_with(conn, ch_type, b_opened_locally, ch_index, svc)
            .map(|channel| channel.cast())
    }
}

/// Shared `UNetConnection::CreateChannel` body.
pub unsafe fn create_channel_with<O: ConnectionReceiveOps>(
    conn: *mut UNetConnection,
    ch_type: i32,
    b_opened_locally: i32,
    ch_index: i32,
    ops: &O,
) -> Result<*mut UChannel, O::Error> {
    unsafe {
        if !ops.known_channel_type(ch_type)? {
            ops.check_fail(c"UChannel::IsKnownChannelType(ChType)", 716)?;
        }
        assert_valid_with(conn, ops)?;
        let mut ch_index = ch_index;
        if ch_index == -1 {
            ch_index = if ch_type == CHTYPE_CONTROL { 0 } else { 1 };
            while ch_index < MAX_CHANNELS && !(*conn).channels[ch_index as usize].is_null() {
                ch_index += 1;
            }
            if ch_index == MAX_CHANNELS {
                return Ok(std::ptr::null_mut());
            }
        }
        if ch_index >= MAX_CHANNELS {
            ops.check_fail(c"ChIndex<MAX_CHANNELS", 733)?;
        }
        if !(*conn).channels[ch_index as usize].is_null() {
            ops.check_fail(c"Channels[ChIndex]==NULL", 734)?;
        }
        let channel = ops.construct_channel(conn.cast(), ch_type)?;
        ops.initialize_channel(ch_type, channel, conn.cast(), ch_index, b_opened_locally)?;
        (*conn).channels[ch_index as usize] = channel.cast();
        ops.add_open_channel(conn.cast(), channel)?;
        Ok(channel)
    }
}

/// `USOCK_Pending` (`UnConn.h:27`).
const USOCK_PENDING: i32 = 2;

/// `Max(A, B)` template from `Core`: `(A >= B) ? A : B`.
fn max_f32(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `UNetConnection::Tick()` (`UnConn.cpp:755-895`).
///
/// # Safety
/// `conn` and every record reached through the operation adapter must remain live.
pub unsafe fn tick(conn: *mut UNetConnection, svc: &Hp2NetConnServices) -> Res<()> {
    unsafe { tick_with(conn, svc) }
}

/// Runs the single source-order connection tick policy through native or Core leaves.
///
/// # Safety
/// `conn` and every record reached through `ops` must remain live for the call.
pub(crate) unsafe fn tick_with<O: ConnectionTickOps>(
    conn: *mut UNetConnection,
    ops: &O,
) -> Result<(), O::Error> {
    unsafe {
        assert_valid_with(conn, ops)?;

        // Lag simulation (`DO_ENABLE_NET_TEST`).
        if (*conn).pkt_lag != 0 {
            let mut index = 0;
            while index < (*conn).delayed.num {
                let packet = ops.delayed_at(conn.cast(), index)?.cast::<DelayedPacket>();
                if ops.seconds() > i64::from_ne_bytes((*packet).send_time) {
                    let data = ops.delayed_data(packet.cast())?;
                    ops.low_level_send(conn.cast(), data, ops.delayed_data_num(packet.cast()))?;
                    ops.delayed_remove(conn.cast(), index)?;
                    index -= 1;
                }
                index += 1;
            }
        }

        // Get frame time.
        let driver = driver_of(conn);
        let current_time = ops.seconds();
        (*conn).frame_time = ftime_sub(current_time, i64::from_ne_bytes((*conn).last_time));
        (*conn).last_time = current_time.to_ne_bytes();
        (*conn).cumulative_time = ftime_add_f32(
            i64::from_ne_bytes((*conn).cumulative_time),
            (*conn).frame_time,
        )
        .to_ne_bytes();
        (*conn).counted_frames = (*conn).counted_frames.wrapping_add(1);
        if ftime_get_float(i64::from_ne_bytes((*conn).cumulative_time)) > 1.0f32 {
            (*conn).average_frame_time =
                ftime_get_float(i64::from_ne_bytes((*conn).cumulative_time))
                    / (*conn).counted_frames as f32;
            (*conn).cumulative_time = 0i64.to_ne_bytes();
            (*conn).counted_frames = 0;
        }

        // Pretend everything was acked, for 100% reliable connections or demo recording.
        if (*conn).internal_ack != 0 {
            (*conn).last_receive_time = (*driver).time;
            let mut index = ops.open_channel_count(conn.cast()) - 1;
            while index >= 0 {
                let channel = ops.open_channel_at(conn.cast(), index)?;
                let mut bunch = (*channel).out_rec.cast::<FOutBunch>();
                while !bunch.is_null() {
                    (*bunch).received_ack = 1;
                    bunch = (*bunch).next.cast::<FOutBunch>();
                }
                (*channel).open_acked = 1;
                ops.channel_received_acks(channel)?;
                index -= 1;
            }
        }

        // Update stats.
        if ftime_sub(
            i64::from_ne_bytes((*driver).time),
            i64::from_ne_bytes((*conn).stat_update_time),
        ) > (*conn).stat_period
        {
            let real_time = ftime_sub(
                i64::from_ne_bytes((*driver).time),
                i64::from_ne_bytes((*conn).stat_update_time),
            );
            (*conn).in_rate = (*conn).in_byte_acc as f32 / real_time;
            (*conn).out_rate = (*conn).out_byte_acc as f32 / real_time;
            (*conn).in_packets = (*conn).in_pkt_acc as f32 / real_time;
            (*conn).out_packets = (*conn).out_pkt_acc as f32 / real_time;
            (*conn).in_bunches = (*conn).in_bun_acc as f32 / real_time;
            (*conn).out_bunches = (*conn).out_bun_acc as f32 / real_time;
            (*conn).in_order = (*conn).in_ord_acc as f32 / real_time;
            (*conn).out_order = (*conn).out_ord_acc as f32 / real_time;
            (*conn).out_loss =
                100.0f32 * (*conn).out_loss_acc as f32 / max_f32((*conn).out_packets, 1.0f32);
            (*conn).in_loss = 100.0f32 * (*conn).in_loss_acc as f32
                / max_f32((*conn).in_packets + (*conn).in_loss_acc as f32, 1.0f32);
            if (*conn).lag_count != 0 {
                (*conn).avg_lag = (*conn).lag_acc / (*conn).lag_count as f32;
            }
            (*conn).best_lag = (*conn).avg_lag;

            // See if we're experiencing high packet loss.
            if (*conn).out_loss > 20.0f32 || (*conn).in_loss > 20.0f32 {
                (*conn).high_loss_count = (*conn).high_loss_count.wrapping_add(1);
            } else {
                (*conn).high_loss_count = 0;
            }

            let actor = (*conn).actor;
            if !actor.is_null() {
                let packet_loss = max_f32((*conn).in_loss, (*conn).out_loss) * 0.01f32;
                let current_net_speed = (*conn).current_net_speed as f32;
                let bad_alert = ((*conn).internal_ack == 0
                    && (((*conn).best_lag as f64 > 1.0
                        || current_net_speed * (1.0f32 - packet_loss) < 2000.0f32)
                        && ops.actor_channel_exists(conn, actor)?))
                    || (*conn).in_packets < 2.0f32;
                ops.set_bad_connection_alert(actor, bad_alert);
            }

            // Init counters.
            (*conn).lag_acc = 0.0f32;
            (*conn).best_lag_acc = 9999.0f32;
            (*conn).in_byte_acc = 0;
            (*conn).out_byte_acc = 0;
            (*conn).in_pkt_acc = 0;
            (*conn).out_pkt_acc = 0;
            (*conn).in_bun_acc = 0;
            (*conn).out_bun_acc = 0;
            (*conn).in_loss_acc = 0;
            (*conn).out_loss_acc = 0;
            (*conn).in_ord_acc = 0;
            (*conn).out_ord_acc = 0;
            (*conn).lag_count = 0;
            (*conn).stat_update_time = (*driver).time;
        }

        // Compute time passed since last update.
        let delta_time = ftime_sub(
            i64::from_ne_bytes((*driver).time),
            i64::from_ne_bytes((*conn).last_tick_time),
        );
        (*conn).last_tick_time = (*driver).time;

        // Update queued byte count.
        let current_net_speed = (*conn).current_net_speed as f32;
        let delta_bytes = current_net_speed * delta_time;
        (*conn).queued_bytes = (*conn).queued_bytes.wrapping_sub(ops.app_round(delta_bytes));
        let allowed_lag = 2.0f32 * delta_bytes;
        if ((*conn).queued_bytes as f32) < -allowed_lag {
            (*conn).queued_bytes = ops.app_round(allowed_lag).wrapping_neg();
        }

        // Handle timeouts.
        let timeout = if (*conn).state == USOCK_PENDING || (*conn).actor.is_null() {
            (*driver).initial_connect_timeout
        } else {
            (*driver).connection_timeout
        };
        let since_receive = ftime_sub(
            i64::from_ne_bytes((*driver).time),
            i64::from_ne_bytes((*conn).last_receive_time),
        );
        if since_receive > timeout {
            if (*conn).state != USOCK_CLOSED {
                ops.log_timeout(timeout, since_receive)?;
            }
            (*conn).state = USOCK_CLOSED;
        } else {
            let mut index = ops.open_channel_count(conn.cast()) - 1;
            while index >= 0 {
                ops.channel_tick(ops.open_channel_at(conn.cast(), index)?)?;
                index -= 1;
            }
            if (*conn).channels[0].is_null()
                && ((*conn).out_reliable[0] != 0 || (*conn).in_reliable[0] != 0)
            {
                (*conn).state = USOCK_CLOSED;
            }
        }

        // Flush.
        purge_acks_with(conn, ops)?;
        if (*conn).time_sensitive != 0
            || ftime_sub(
                i64::from_ne_bytes((*driver).time),
                i64::from_ne_bytes((*conn).last_send_time),
            ) > (*driver).keep_alive_time
        {
            ops.flush_net(conn.cast())?;
        }

        // `Download` is read after `FlushNet`, as in the C++ `TickDownload` guard.
        let download = (*conn).download;
        if !download.is_null() {
            ops.download_tick(download)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp2_abi::engine::generated::TArrayRaw;
    use hp2_core::array_storage::{self, Hp2ArrayServices};
    use std::sync::Mutex;

    static ALLOCATOR_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" {
        #[link_name = "realloc"]
        fn native_realloc(data: *mut c_void, bytes: usize) -> *mut c_void;
        #[link_name = "free"]
        fn native_free(data: *mut c_void);
    }

    unsafe extern "C" fn realloc(
        _: *mut c_void,
        data: *mut c_void,
        bytes: usize,
        _: *const i32,
        _: i32,
        _: *const c_void,
        out: *mut *mut c_void,
    ) -> i32 {
        // SAFETY: every owned test buffer uses the same native C allocator.
        unsafe {
            *out = native_realloc(data, bytes.max(1));
            i32::from((*out).is_null())
        }
    }

    unsafe extern "C" fn checked_size(
        _: *mut c_void,
        bytes: usize,
        out: *mut i32,
    ) -> i32 {
        let Ok(bytes) = i32::try_from(bytes) else { return 1 };
        // SAFETY: the canonical allocator supplies a writable result slot.
        unsafe { *out = bytes };
        0
    }

    unsafe extern "C" fn free(_: *mut c_void, data: *mut c_void) -> i32 {
        // SAFETY: buffers came from native_realloc and are released once.
        unsafe { native_free(data) };
        0
    }

    fn publish_allocator() {
        // SAFETY: all tests using this process-global allocator hold ALLOCATOR_LOCK,
        // and the native allocation leaves are valid for the process lifetime.
        unsafe {
            array_storage::publish_allocator(Hp2ArrayServices {
                context: std::ptr::null_mut(),
                realloc,
                checked_int_size: checked_size,
                free,
            });
        }
    }

    fn reader(storage: &mut MaybeUninit<FBitReader>) -> *mut FBitReader {
        let reader = storage.as_mut_ptr();
        // SAFETY: only the fields used by SetData/SerializeBits are exposed;
        // this fixture never reads or invents a native vtable or opaque padding.
        unsafe {
            (&raw mut (*reader).buffer).write(TArrayRaw {
                data: std::ptr::null_mut(),
                num: 0,
                max: 0,
            });
            (*reader).num = 0;
            (*reader).pos = 0;
            (*reader).ar_is_error = 0;
        }
        reader
    }

    #[test]
    fn connection_bunch_set_data_owns_unaligned_payload() {
        let _lock = ALLOCATOR_LOCK.lock().expect("allocator test lock poisoned");
        publish_allocator();
        let mut source_storage = MaybeUninit::<FBitReader>::uninit();
        let mut bunch_storage = MaybeUninit::<FBitReader>::uninit();
        let source = reader(&mut source_storage);
        let bunch = reader(&mut bunch_storage);
        let mut bytes = [0b1101_0110u8, 0b1010_1101];
        // SAFETY: source borrows two stack bytes, destination owns its canonical
        // buffer, and both remain live through serialization and release.
        unsafe {
            (*source).buffer.data = bytes.as_mut_ptr().cast();
            (*source).buffer.num = 2;
            (*source).buffer.max = 2;
            (*source).num = 16;
            (*source).pos = 3;
            (*bunch).ar_is_error = 1;
            set_bunch_data(bunch, source, 9).expect("payload copy succeeds");
            assert_eq!((*source).pos, 12);
            assert_eq!((*bunch).num, 9);
            assert_eq!((*bunch).pos, 0);
            assert_eq!((*bunch).ar_is_error, 0);
            assert_eq!(std::slice::from_raw_parts((*bunch).buffer.data.cast::<u8>(), 2),
                &[0b1011_1010, 1]);
            assert_ne!((*bunch).buffer.data, (*source).buffer.data);
            bytes.fill(0);
            assert_eq!(*(*bunch).buffer.data.cast::<u8>(), 0b1011_1010);
            net_bunch::release_buffer(&raw mut (*bunch).buffer).expect("release payload");
            assert!((*bunch).buffer.data.is_null());
        }
    }

    #[test]
    fn connection_bunch_set_data_preserves_reader_overflow_behavior() {
        let _lock = ALLOCATOR_LOCK.lock().expect("allocator test lock poisoned");
        publish_allocator();
        let mut source_storage = MaybeUninit::<FBitReader>::uninit();
        let mut bunch_storage = MaybeUninit::<FBitReader>::uninit();
        let source = reader(&mut source_storage);
        let bunch = reader(&mut bunch_storage);
        // SAFETY: an empty source triggers overflow before any source byte read;
        // destination storage is allocated and released by the canonical owner.
        unsafe {
            set_bunch_data(bunch, source, 9).expect("overflow is archive state, not a throw");
            assert_eq!((*source).ar_is_error, 1);
            assert_eq!((*source).pos, 0);
            assert_eq!((*bunch).ar_is_error, 0);
            assert_eq!((*bunch).num, 9);
            assert_eq!((*bunch).pos, 0);
            net_bunch::release_buffer(&raw mut (*bunch).buffer).expect("release payload");
        }
    }
}
