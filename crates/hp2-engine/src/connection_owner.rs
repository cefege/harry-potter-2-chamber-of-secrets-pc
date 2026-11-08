//! Canonical Rust ownership of Engine's abstract UNetConnection record and packet policy.

use crate::net_conn_abi::{ConnectionPacketOps, ConnectionReceiveFileOps, ConnectionReceiveOps, ConnectionTickOps};
use hp2_abi::engine::generated::{
    APlayerPawn, DelayedPacket, FBitReader, FBitWriter, FDownloadInfo, FInBunch, FOutBunch, FURL,
    TArrayRaw, UChannel, UClass, UDownload, UPackageMapLevel, UPlayer, UNetConnection, UNetDriver, UObject,
};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use hp2_core::{array_storage, bits, map_storage};
use std::alloc::Layout;
use std::ffi::{CStr, c_void};
use std::mem;
use std::ptr;

const CLASS_ABSTRACT: u32 = 0x0000_0001;
const CLASS_TRANSIENT: u32 = 0x0000_0008;
const CLASS_CONFIG: u32 = 0x0000_0004;
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const BASE: ClassOps = crate::player_owner::OPERATIONS;
const ARRAY_TAG: [u32; 1] = [0];
const PKT_LOSS: [i32; 9] = [80, 107, 116, 76, 111, 115, 115, 61, 0];
const PKT_ORDER: [i32; 10] = [80, 107, 116, 79, 114, 100, 101, 114, 61, 0];
const PKT_DUP: [i32; 8] = [80, 107, 116, 68, 117, 112, 61, 0];
const PKT_LAG: [i32; 8] = [80, 107, 116, 76, 97, 103, 61, 0];
const LAN_OPTION: [u32; 4] = [76, 65, 78, 0];
const DEMO_CONNECTION: &str = "Engine.DemoRecConnection";

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "NetConnection", package: "Engine", super_name: Some("Player"), within: "Object",
    flags: CLASS_ABSTRACT | CLASS_TRANSIENT | CLASS_CONFIG, object_flags: OBJECT_FLAGS,
    config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UNetConnection>()), operations: Some(CONNECTION_OPERATIONS),
    properties: &[], defaults: &[],
}];

pub(crate) const CONNECTION_OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.construct = construct;
    operations.destroy = destroy;
    operations.finalize_members = finalize_connection;
    operations.serialize = serialize;
    operations
};

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (BASE.construct)(access, object)?; }
    let conn = object.cast::<UNetConnection>();
    unsafe { construct_members(conn)?; }
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    unsafe { crate::url_owner::UrlOwner::construct_default(access, &defaults, ptr::addr_of_mut!((*conn).url))?; }
    Ok(())
}

unsafe fn construct_members(conn: *mut UNetConnection) -> RuntimeResult<()> {
    let out = unsafe { ptr::addr_of_mut!((*conn).out) };
    unsafe {
        ptr::addr_of_mut!((*out).buffer).write(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 });
        hp2_core::archive_state::initialize_scalar_state(out.cast());
        map_storage::initialize_owned_pod(ptr::addr_of_mut!((*conn).actor_channels).cast())?;
        if bits::writer_init(out, 0, array_storage::system_allocator()).is_err() {
            map_storage::destroy_owned_pod(ptr::addr_of_mut!((*conn).actor_channels).cast());
            return Err(RuntimeError::new("Failed to initialize connection output writer"));
        }
        if crate::net_bunch::construct_out(ptr::addr_of_mut!((*conn).last_out)).is_err() {
            crate::net_bunch::release_core_buffer(ptr::addr_of_mut!((*conn).out.buffer))
                .map_err(|_| RuntimeError::new("Failed to release connection output writer"))?;
            map_storage::destroy_owned_pod(ptr::addr_of_mut!((*conn).actor_channels).cast());
            return Err(RuntimeError::new("Failed to initialize last outgoing bunch"));
        }
    }
    Ok(())
}

/// Invokes the true `UNetConnection(Driver, URL)` base constructor body for a
/// canonical runtime connection; the Core constructor policy owns publication.
///
/// # Safety
/// `object`, `driver`, and `url` are live canonical records; the object has not
/// yet had any class constructor invoked.
pub(crate) unsafe fn construct_parameterized(
    access: CoreAccess<'_>,
    object: *mut UObject,
    driver: *mut UNetDriver,
    url: *const FURL,
) -> RuntimeResult<()> {
    unsafe { (BASE.construct)(access, object)?; }
    let conn = object.cast::<UNetConnection>();
    unsafe { construct_members(conn)?; }
    unsafe {
        (*conn).driver = driver.cast();
        (*conn).state = 0;
        (*conn).protocol_version = 1;
        (*conn).last_receive_time = (*driver).time;
        (*conn).last_send_time = (*driver).time;
        (*conn).last_tick_time = (*driver).time;
        (*conn).stat_update_time = (*driver).time;
        (*conn).queued_bytes = 0;
        (*conn).stat_period = 1.0;
        (*conn).out_ack_packet_id = -1;
        (*conn).in_packet_id = -1;
        (*conn).lag_acc = 9999.0;
        (*conn).best_lag_acc = 9999.0;
        (*conn).best_lag = 9999.0;
        (*conn).avg_lag = 9999.0;
        (*conn).negotiated_ver = 400;
        (*conn).remote_version = 0;
        (*conn).pkt_loss = parse_test_option(access.command_line(), &PKT_LOSS);
        (*conn).pkt_order = parse_test_option(access.command_line(), &PKT_ORDER);
        (*conn).pkt_dup = parse_test_option(access.command_line(), &PKT_DUP);
        (*conn).pkt_lag = parse_test_option(access.command_line(), &PKT_LAG);
    }
    unsafe {
        crate::url_owner::UrlOwner::copy_construct(
            access,
            ptr::addr_of_mut!((*conn).url),
            url,
        )?;
    }
    let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
    let is_lan = unsafe {
        crate::url_owner::UrlOwner::has_option(
            access, &defaults, ptr::addr_of!((*conn).url), LAN_OPTION.as_ptr(),
        )?
    };
    let player_class = access.native_class("Engine.Player")?;
    let player_default = unsafe { access.default_object(player_class)? }.cast::<UPlayer>();
    unsafe {
        (*conn).current_net_speed = if is_lan {
            (*player_default).configured_lan_speed
        } else {
            (*player_default).configured_internet_speed
        };
    }

    let package_map_class = access.native_class("Engine.PackageMapLevel")?;
    let package_map = unsafe {
        access.construct_object(package_map_class, object, 0, 0, ptr::null_mut())?
    };
    unsafe {
        (*package_map.cast::<UPackageMapLevel>()).connection = object.cast();
        (*conn).package_map = package_map.cast();
    }
    Ok(())
}

fn parse_test_option(command_line: &[u32], key: &[i32]) -> i32 {
    let found = unsafe { hp2_core::search::strfind(command_line.as_ptr().cast(), key.as_ptr()) };
    if found.is_null() { return 0; }
    let mut cursor = unsafe { found.add(key.len() - 1) };
    while unsafe { matches!(*cursor, 9..=13 | 32) } { cursor = unsafe { cursor.add(1) }; }
    let negative = unsafe { *cursor == b'-' as i32 };
    if negative || unsafe { *cursor == b'+' as i32 } { cursor = unsafe { cursor.add(1) }; }
    let mut value = 0i32;
    while unsafe { (b'0' as i32..=b'9' as i32).contains(&*cursor) } {
        value = value.wrapping_mul(10).wrapping_add(unsafe { *cursor - b'0' as i32 });
        cursor = unsafe { cursor.add(1) };
    }
    if negative { value.wrapping_neg() } else { value }
}
/// Initializes the canonical Out writer at the source `UNetConnection::InitOut` point.
///
/// # Safety
/// `conn` is a live connection with an owned output writer.
pub(crate) unsafe fn init_out(access: CoreAccess<'_>, conn: *mut UNetConnection) -> RuntimeResult<()> {
    let ops = CorePacketOps { access };
    unsafe { ops.init_out(conn.cast()) }
}

unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let conn = object.cast::<UNetConnection>();
    unsafe { crate::net_conn::destroy_with_ops(conn.cast(), &mut CoreConnectionDestroyOps(access)) }
}

unsafe fn finalize_connection(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UNetConnection")); }
    let conn = object.cast::<UNetConnection>();
    unsafe {
        let mut index = 0;
        while index < (*conn).delayed.num {
            let packet = (*conn).delayed.data.cast::<DelayedPacket>().add(index as usize);
            release_array(ptr::addr_of_mut!((*packet).data))?;
            index += 1;
        }
        index = 0;
        while index < (*conn).download_info.num {
            let info = (*conn).download_info.data.cast::<FDownloadInfo>().add(index as usize);
            release_array(ptr::addr_of_mut!((*info).class_name))?;
            release_array(ptr::addr_of_mut!((*info).params))?;
            index += 1;
        }
        crate::url_owner::UrlOwner::destroy(access, ptr::addr_of_mut!((*conn).url))?;
        release_array(ptr::addr_of_mut!((*conn).request_url))?;
        release_array(ptr::addr_of_mut!((*conn).queued_acks))?;
        release_array(ptr::addr_of_mut!((*conn).resend_acks))?;
        release_array(ptr::addr_of_mut!((*conn).open_channels))?;
        release_array(ptr::addr_of_mut!((*conn).sent_temporaries))?;
        release_array(ptr::addr_of_mut!((*conn).download_info))?;
        release_array(ptr::addr_of_mut!((*conn).delayed))?;
        map_storage::destroy_owned_pod(ptr::addr_of_mut!((*conn).actor_channels).cast());
    }
    unsafe {
        crate::net_bunch::release_core_buffer(ptr::addr_of_mut!((*conn).out.buffer))
            .map_err(|_| RuntimeError::new("Failed to release connection output writer"))?;
        crate::net_bunch::destruct_out(ptr::addr_of_mut!((*conn).last_out))
            .map_err(|_| RuntimeError::new("Failed to release last outgoing bunch"))?;
    }
    Ok(())
}

struct CoreConnectionDestroyOps<'a>(CoreAccess<'a>);

impl crate::net_conn_abi::ConnectionDestroyOps for CoreConnectionDestroyOps<'_> {
    type Error = RuntimeError;

    unsafe fn log_close(&self, conn: *mut c_void) -> RuntimeResult<()> {
        let access = self.0;
        let category = access.intern_str("NetComeGo", hp2_core::name_owner::FindName::Add)?;
        let object = conn.cast::<UObject>();
        let name = access.name_text(i32::from_ne_bytes(unsafe { (*object).name }))?;
        let mut now: libc::time_t = 0;
        unsafe { libc::time(&mut now); }
        let mut broken_time: libc::tm = unsafe { mem::zeroed() };
        if unsafe { libc::localtime_r(&now, &mut broken_time) }.is_null() {
            return Err(RuntimeError::new("Unable to format connection close timestamp"));
        }
        let mut timestamp = [0i32; 40];
        if !unsafe {
            hp2_core::unix_clock::format_timestamp(
                broken_time.tm_wday, broken_time.tm_mon, broken_time.tm_mday,
                broken_time.tm_hour, broken_time.tm_min, broken_time.tm_sec,
                broken_time.tm_year, timestamp.as_mut_ptr(), timestamp.len(),
            )
        } {
            return Err(RuntimeError::new("Unable to format connection close timestamp"));
        }
        let mut message = Vec::with_capacity(name.len() + 7 + timestamp.len());
        message.extend([67, 108, 111, 115, 101, 32]);
        message.extend(name.into_iter().take_while(|&unit| unit != 0));
        message.push(32);
        message.extend(timestamp.into_iter().take_while(|&unit| unit != 0).map(|unit| unit as u32));
        access.emit_log(category, &message)
    }

    unsafe fn close_channel_zero(&self, conn: *mut c_void) -> RuntimeResult<()> {
        let channel = unsafe { (*conn.cast::<UNetConnection>()).channels[0] };
        if !channel.is_null() {
            unsafe { crate::channel_owner::close(self.0, channel.cast())?; }
        }
        Ok(())
    }
    unsafe fn flush_net(&self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe { flush_net(self.0, conn.cast()) }
    }
    unsafe fn unregister(&self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            let conn = conn.cast::<UNetConnection>();
            let driver = (*conn).driver.cast::<UNetDriver>();
            let server = (*driver).server_connection;
            if !server.is_null() {
                if server != conn.cast() {
                    return Err(RuntimeError::new("Driver ServerConnection does not refer to this connection"));
                }
                (*driver).server_connection = ptr::null_mut();
                return Ok(());
            }
            if !(*driver).server_connection.is_null() {
                return Err(RuntimeError::new("Driver ServerConnection must be null"));
            }
            let clients = ptr::addr_of_mut!((*driver).client_connections);
            let mut removed = 0;
            let mut index = 0;
            while index < (*clients).num {
                let current = *(*clients).data.cast::<*mut c_void>().add(index as usize);
                if current == conn.cast() {
                    array_storage::remove(
                        array_slots(clients), index, 1, mem::size_of::<*mut c_void>() as i32,
                        ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator(),
                    ).map_err(|_| RuntimeError::new("Failed to remove connection from driver"))?;
                    removed += 1;
                } else {
                    index += 1;
                }
            }
            if removed != 1 {
                return Err(RuntimeError::new("Driver client removal count is not one"));
            }
        }
        Ok(())
    }
    unsafe fn set_closed(&self, conn: *mut c_void) {
        unsafe { (*conn.cast::<UNetConnection>()).state = 1; }
    }
    unsafe fn delete_open_channels(&self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            let connection = conn.cast::<UNetConnection>();
            let open = ptr::addr_of_mut!((*connection).open_channels);
            let mut index = (*open).num - 1;
            while index >= 0 {
                let channel = *(*open).data.cast::<*mut UObject>().add(index as usize);
                if !channel.is_null() {
                    self.0.conditional_destroy(channel)?;
                    self.0.delete_object(channel)?;
                }
                index -= 1;
            }
        }
        Ok(())
    }
    unsafe fn delete_package_map(&self, conn: *mut c_void) -> RuntimeResult<()> {
        let object = unsafe { (*conn.cast::<UNetConnection>()).package_map.cast::<UObject>() };
        if !object.is_null() {
            unsafe { self.0.conditional_destroy(object)?; self.0.delete_object(object)?; }
        }
        Ok(())
    }
    unsafe fn delete_download(&self, conn: *mut c_void) -> RuntimeResult<()> {
        let object = unsafe { (*conn.cast::<UNetConnection>()).download.cast::<UObject>() };
        if !object.is_null() {
            unsafe { self.0.conditional_destroy(object)?; self.0.delete_object(object)?; }
        }
        Ok(())
    }
    unsafe fn super_destroy(&self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe { (BASE.destroy)(self.0, conn.cast()) }
    }
}

unsafe fn serialize(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &hp2_core::object_archive::ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe { (BASE.serialize)(access, object, archive)?; }
    let conn = object.cast::<UNetConnection>();
    unsafe {
        archive.serialize_object_slot(ptr::addr_of_mut!((*conn).package_map).cast())?;
        for channel in &mut (*conn).channels {
            archive.serialize_object_slot((channel as *mut *mut c_void).cast())?;
        }
        archive.serialize_object_slot(ptr::addr_of_mut!((*conn).download).cast())?;
    }
    Ok(())
}


/// Runs the Core base `UNetConnection::IsNetReady` policy, honoring the
/// `UDemoRecConnection` override before reading the base packet counters.
///
/// # Safety
/// `connection` must be a live canonical connection with initialized packet state.
pub unsafe fn is_net_ready(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    saturate: i32,
) -> RuntimeResult<i32> {
    if connection.is_null() {
        return Err(RuntimeError::new("Null canonical UNetConnection"));
    }
    if unsafe { is_demo_connection(access, connection)? } {
        return Ok(1);
    }
    let ops = CorePacketOps { access };
    Ok(unsafe { crate::net_conn::is_net_ready_with(connection, saturate, &ops) })
}

/// Ticks a canonical connection with the same timeout, stats, channel, flush and download policy as native.
///
/// # Safety
/// `connection` and all records reachable through its owners must remain live.
pub unsafe fn tick(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
) -> RuntimeResult<()> {
    if connection.is_null() {
        return Err(RuntimeError::new("Null canonical UNetConnection"));
    }
    unsafe { crate::net_conn::tick_with(connection, &CorePacketOps { access }) }
}
/// Flushes packets through source-order policy and the concrete connection transport.
///
/// # Safety
/// `conn` must point to a live canonical UNetConnection with initialized packet state.
pub unsafe fn flush_net(access: CoreAccess<'_>, conn: *mut UNetConnection) -> RuntimeResult<()> {
    let is_demo = unsafe { is_demo_connection(access, conn)? };
    let driver = unsafe { (*conn).driver.cast::<UNetDriver>() };
    if is_demo && !unsafe { (*driver).server_connection.is_null() } {
        return Ok(());
    }
    let ops = CorePacketOps { access };
    unsafe { crate::net_conn::flush_net_with(conn, &ops) }
}

/// Checks the actual native class ancestry for the FILE-transport connection.
///
/// # Safety
/// `conn` must be a live canonical connection with a valid UObject class chain.
pub(crate) unsafe fn is_demo_connection(access: CoreAccess<'_>, conn: *mut UNetConnection) -> RuntimeResult<bool> {
    let demo_class = access.native_class(DEMO_CONNECTION)?;
    let mut class = unsafe { (*conn.cast::<UObject>()).class.cast::<UClass>() };
    while !class.is_null() {
        if class.cast::<UObject>() == demo_class.cast() {
            return Ok(true);
        }
        class = unsafe { (*class).super_field.cast::<UClass>() };
    }
    Ok(false)
}

/// Serializes and sends one bunch using the shared native packet algorithm.
///
/// # Safety
/// `conn` and `bunch` must be live canonical records with initialized Out storage.
pub unsafe fn send_raw_bunch(
    access: CoreAccess<'_>, conn: *mut UNetConnection, bunch: *mut FOutBunch, allow_merge: bool,
) -> RuntimeResult<i32> {
    let ops = CorePacketOps { access };
    unsafe { crate::net_conn::send_raw_bunch_with(conn, bunch, i32::from(allow_merge), &ops) }
}

/// Receives one raw datagram through the shared packet parser and canonical
/// reader/channel operations.
///
/// # Safety
/// `connection` is live and `data` covers `count` readable bytes when positive.
pub unsafe fn received_raw_packet(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    data: *const u8,
    count: i32,
) -> RuntimeResult<()> {
    unsafe { crate::net_conn::received_raw_packet_with(connection, data, count, &CorePacketOps { access }) }
}

/// Creates, initializes and publishes one canonical channel through the shared
/// `UNetConnection::CreateChannel` policy.
///
/// # Safety
/// `connection` is live and its channel/open-channel storage is initialized.
pub unsafe fn create_channel(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    channel_type: i32,
    opened_locally: i32,
    index: i32,
) -> RuntimeResult<*mut UChannel> {
    unsafe {
        crate::net_conn::create_channel_with(
            connection,
            channel_type,
            opened_locally,
            index,
            &CorePacketOps { access },
        )
    }
}

/// Starts receiving a package through the shared connection ReceiveFile policy.
///
/// # Safety
/// `connection` and its package map/download-info storage must be live canonical objects.
pub unsafe fn receive_file(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    package_index: i32,
) -> RuntimeResult<()> {
    unsafe {
        crate::net_conn::receive_file_with(
            connection,
            package_index,
            &CoreReceiveFileOps { access },
        )
    }
}

struct CorePacketOps<'a> { access: CoreAccess<'a> }

impl ConnectionPacketOps for CorePacketOps<'_> {
    type Error = RuntimeError;

    unsafe fn check_fail(&self, expression: &CStr, line: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(&format!("{} (UnConn.cpp:{line})", expression.to_string_lossy())))
    }
    unsafe fn out(&self, conn: *mut c_void) -> *mut c_void {
        unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).out).cast() }
    }
    unsafe fn writer_num_bits(&self, writer: *mut c_void) -> i32 {
        unsafe { bits::writer_get_num_bits(writer.cast()) }
    }
    unsafe fn writer_num_bytes(&self, writer: *mut c_void) -> i32 {
        unsafe { bits::writer_get_num_bytes(writer.cast()) }
    }
    unsafe fn writer_data(&self, writer: *mut c_void) -> *mut c_void {
        unsafe { bits::writer_get_data(writer.cast()).cast() }
    }
    unsafe fn writer_is_error(&self, writer: *mut c_void) -> bool {
        unsafe { (*writer.cast::<FBitWriter>()).ar_is_error != 0 }
    }
    unsafe fn writer_write_bit(&self, writer: *mut c_void, bit: i32) -> RuntimeResult<()> {
        unsafe { bits::writer_write_bit(writer.cast(), bit as u8); }
        Ok(())
    }
    unsafe fn writer_write_int(&self, writer: *mut c_void, value: i32, max: i32) -> RuntimeResult<()> {
        unsafe {
            bits::writer_write_int_with_checker(
                writer.cast(),
                value as u32,
                max as u32,
                hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks,
                |expression, line| {
                    let expression = CStr::from_bytes_with_nul(expression)
                        .map_err(|_| RuntimeError::new("Invalid bit-writer assertion expression"))?;
                    Err(RuntimeError::new(&format!(
                        "{} (UnBits.cpp:{line})",
                        expression.to_string_lossy()
                    )))
                },
            )
        }
    }
    unsafe fn writer_serialize_bits(&self, writer: *mut c_void, data: *const c_void, count: i32) -> RuntimeResult<()> {
        unsafe { bits::writer_serialize_bits(writer.cast(), data.cast(), count); }
        Ok(())
    }
    unsafe fn reset_last_end(&self, conn: *mut c_void) {
        unsafe { (*conn.cast::<UNetConnection>()).last_end.num = 0; }
    }
    unsafe fn mark_last_start(&self, conn: *mut c_void) {
        let conn = conn.cast::<UNetConnection>();
        unsafe {
            (*conn).last_start.num = (*conn).out.num;
            (*conn).last_start.overflowed = (*conn).out.ar_is_error;
        }
    }
    unsafe fn pre_send_overflow(&self, num_bits: i32, size_bits: i32, max_bits: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(&format!("Connection packet overflow: {num_bits}+{size_bits}+32>{max_bits}")))
    }
    unsafe fn low_level_send(&self, conn: *mut c_void, data: *const c_void, bytes: i32) -> RuntimeResult<()> {
        if !unsafe { is_demo_connection(self.access, conn.cast())? } {
            return Err(RuntimeError::new("Canonical packet transport requires a DemoRecConnection class"));
        }
        unsafe { crate::demo_owner::low_level_send(self.access, conn.cast(), data.cast(), bytes) }
    }
    fn random(&self) -> f32 {
        self.access.rand() as f32 / hp2_core::ansi_trace::RAND_MAX as f32
    }
    fn ceil_log_two(&self, value: u32) -> i32 {
        i32::from(hp2_core::bits::ceil_log_two(value))
    }
    fn seconds(&self) -> i64 {
        hp2_core::time::from_double(hp2_core::unix_clock::seconds_slow())
    }
    unsafe fn init_out(&self, conn: *mut c_void) -> RuntimeResult<()> {
        let conn = conn.cast::<UNetConnection>();
        let writer = unsafe { ptr::addr_of_mut!((*conn).out) };
        unsafe {
            crate::net_bunch::release_core_buffer(&mut (*writer).buffer)
                .map_err(|_| RuntimeError::new("Failed to release connection packet buffer"))?;
            bits::writer_init(writer, (*conn).max_packet.wrapping_mul(8), array_storage::system_allocator())
                .map_err(|_| RuntimeError::new("Failed to initialize connection packet buffer"))?;
        }
        Ok(())
    }
    unsafe fn flush_net(&self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe { flush_net(self.access, conn.cast()) }
    }
    unsafe fn queued_ack_count(&self, conn: *mut c_void) -> i32 {
        unsafe { (*conn.cast::<UNetConnection>()).queued_acks.num }
    }
    unsafe fn queued_ack(&self, conn: *mut c_void, index: i32) -> RuntimeResult<i32> {
        let acks = unsafe { &(*conn.cast::<UNetConnection>()).queued_acks };
        if index < 0 || index >= acks.num || acks.data.is_null() {
            return Err(RuntimeError::new("Queued acknowledgement index is invalid"));
        }
        Ok(unsafe { *acks.data.cast::<i32>().add(index as usize) })
    }
    unsafe fn add_queued_ack(&self, conn: *mut c_void, ack: i32) -> RuntimeResult<()> {
        let conn = conn.cast::<UNetConnection>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*conn).queued_acks)) };
        unsafe { array_storage::add(slots, 1, 4, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to grow queued acknowledgement array"))?;
        let index = unsafe { (*conn).queued_acks.num - 1 };
        unsafe { *(*conn).queued_acks.data.cast::<i32>().add(index as usize) = ack; }
        Ok(())
    }
    unsafe fn resend_ack_count(&self, conn: *mut c_void) -> i32 {
        unsafe { (*conn.cast::<UNetConnection>()).resend_acks.num }
    }
    unsafe fn resend_ack(&self, conn: *mut c_void, index: i32) -> RuntimeResult<i32> {
        let acks = unsafe { &(*conn.cast::<UNetConnection>()).resend_acks };
        if index < 0 || index >= acks.num || acks.data.is_null() {
            return Err(RuntimeError::new("Resend acknowledgement index is invalid"));
        }
        Ok(unsafe { *acks.data.cast::<i32>().add(index as usize) })
    }
    unsafe fn add_resend_ack(&self, conn: *mut c_void, ack: i32) -> RuntimeResult<()> {
        let conn = conn.cast::<UNetConnection>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*conn).resend_acks)) };
        unsafe { array_storage::add(slots, 1, 4, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to grow resend acknowledgement array"))?;
        let index = unsafe { (*conn).resend_acks.num - 1 };
        unsafe { *(*conn).resend_acks.data.cast::<i32>().add(index as usize) = ack; }
        Ok(())
    }
    unsafe fn empty_queued_acks(&self, conn: *mut c_void, slack: i32) -> RuntimeResult<()> {
        let conn = conn.cast::<UNetConnection>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*conn).queued_acks)) };
        unsafe { array_storage::empty(slots, slack, 4, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to clear queued acknowledgements"))
    }
    unsafe fn empty_resend_acks(&self, conn: *mut c_void, slack: i32) -> RuntimeResult<()> {
        let conn = conn.cast::<UNetConnection>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*conn).resend_acks)) };
        unsafe { array_storage::empty(slots, slack, 4, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to clear resend acknowledgements"))
    }
    unsafe fn delayed_add(&self, conn: *mut c_void) -> RuntimeResult<*mut c_void> {
        let conn = conn.cast::<UNetConnection>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*conn).delayed)) };
        let index = unsafe { array_storage::add_zeroed(slots, 1, mem::size_of::<DelayedPacket>() as i32, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to grow delayed packet array"))?;
        let ptr = unsafe { (*conn).delayed.data.cast::<DelayedPacket>().add(index as usize) };
        Ok(ptr.cast())
    }
    unsafe fn delayed_at(&self, conn: *mut c_void, index: i32) -> RuntimeResult<*mut c_void> {
        let delayed = unsafe { &mut (*conn.cast::<UNetConnection>()).delayed };
        if index < 0 || index >= delayed.num || delayed.data.is_null() { return Err(RuntimeError::new("Delayed packet index is invalid")); }
        Ok(unsafe { delayed.data.cast::<DelayedPacket>().add(index as usize).cast() })
    }
    unsafe fn delayed_data_add(&self, packet: *mut c_void, bytes: i32) -> RuntimeResult<()> {
        let packet = packet.cast::<DelayedPacket>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*packet).data)) };
        unsafe { array_storage::set(slots, bytes, 1, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to allocate delayed packet bytes"))?;
        unsafe { (*packet).data.num = bytes; }
        Ok(())
    }
    unsafe fn delayed_data(&self, packet: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*packet.cast::<DelayedPacket>()).data.data.cast() })
    }
    unsafe fn delayed_data_num(&self, packet: *mut c_void) -> i32 {
        unsafe { (*packet.cast::<DelayedPacket>()).data.num }
    }
    unsafe fn delayed_remove(&self, conn: *mut c_void, index: i32) -> RuntimeResult<()> {
        let conn = conn.cast::<UNetConnection>();
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*conn).delayed)) };
        if index < 0 || index >= unsafe { (*conn).delayed.num } { return Err(RuntimeError::new("Delayed packet index is invalid")); }
        let packet = unsafe { (*conn).delayed.data.cast::<DelayedPacket>().add(index as usize) };
        unsafe {
            crate::net_bunch::release_core_buffer(ptr::addr_of_mut!((*packet).data))
                .map_err(|_| RuntimeError::new("Failed to release delayed packet bytes"))?;
            array_storage::remove(slots, index, 1, mem::size_of::<DelayedPacket>() as i32, ARRAY_TAG.as_ptr().cast(), array_storage::system_allocator())
        }.map_err(|_| RuntimeError::new("Failed to remove delayed packet"))
    }
    unsafe fn writer_construct(&self, writer: *mut FBitWriter) -> RuntimeResult<()> {
        unsafe {
            ptr::write_bytes(writer.cast::<u8>(), 0, mem::size_of::<usize>());
            ptr::addr_of_mut!((*writer).buffer).write(hp2_abi::engine::generated::TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 });
            hp2_core::archive_state::initialize_scalar_state(writer.cast());
            (*writer).num = 0;
            (*writer).max = 0;
        }
        Ok(())
    }
    unsafe fn writer_destruct(&self, _writer: *mut FBitWriter) -> RuntimeResult<()> { Ok(()) }
    unsafe fn writer_initialize(&self, writer: *mut FBitWriter, capacity: i32) -> RuntimeResult<()> {
        unsafe { bits::writer_init(writer, capacity, array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to initialize packet header writer"))
    }
    unsafe fn writer_release_buffer(&self, writer: *mut FBitWriter) -> RuntimeResult<()> {
        unsafe { crate::net_bunch::release_core_buffer(ptr::addr_of_mut!((*writer).buffer)) }
            .map_err(|_| RuntimeError::new("Failed to release packet header buffer"))
    }
    unsafe fn out_bunch_is_error(&self, bunch: *mut FOutBunch) -> bool {
        unsafe { (*bunch).base.ar_is_error != 0 }
    }
}

impl ConnectionReceiveOps for CorePacketOps<'_> {
    unsafe fn reader_construct(&self, reader: *mut FBitReader) -> RuntimeResult<()> {
        unsafe {
            ptr::write_bytes(reader.cast::<u8>(), 0, mem::size_of::<usize>());
            ptr::addr_of_mut!((*reader).buffer).write(TArrayRaw {
                data: ptr::null_mut(), num: 0, max: 0,
            });
            hp2_core::archive_state::initialize_scalar_state(reader.cast());
            (*reader).num = 0;
            (*reader).pos = 0;
            (*reader).ar_is_error = 0;
            bits::reader_init(reader, ptr::null(), 0, array_storage::system_allocator())
                .map_err(|_| RuntimeError::new("Failed to construct packet bit reader"))?;
        }
        Ok(())
    }
    unsafe fn reader_initialize(
        &self,
        reader: *mut FBitReader,
        data: *const u8,
        bit_count: i32,
    ) -> RuntimeResult<()> {
        unsafe { bits::reader_init(reader, data, bit_count, array_storage::system_allocator()) }
            .map_err(|_| RuntimeError::new("Failed to initialize packet bit reader"))
    }
    unsafe fn reader_release(&self, reader: *mut FBitReader) -> RuntimeResult<()> {
        unsafe { crate::net_bunch::release_core_buffer(ptr::addr_of_mut!((*reader).buffer)) }
            .map_err(|_| RuntimeError::new("Failed to release packet bit reader"))
    }
    unsafe fn reader_destruct(&self, _reader: *mut FBitReader) -> RuntimeResult<()> { Ok(()) }
    unsafe fn reader_is_error(&self, reader: *mut FBitReader) -> bool {
        unsafe { (*reader).ar_is_error != 0 }
    }
    unsafe fn reader_at_end(&self, reader: *mut FBitReader) -> bool {
        unsafe { bits::reader_at_end(reader) != 0 }
    }
    unsafe fn reader_read_bit(&self, reader: *mut FBitReader) -> i32 {
        unsafe { i32::from(bits::reader_read_bit(reader)) }
    }
    unsafe fn reader_read_int(&self, reader: *mut FBitReader, max: i32) -> i32 {
        unsafe { bits::reader_read_int(reader, max as u32) as i32 }
    }
    unsafe fn bunch_construct(&self, bunch: *mut FInBunch) -> RuntimeResult<()> {
        unsafe { crate::net_bunch::construct_in_core(bunch) }
            .map_err(|_| RuntimeError::new("Failed to construct canonical incoming bunch"))
    }
    unsafe fn bunch_destruct(&self, _bunch: *mut FInBunch) -> RuntimeResult<()> { Ok(()) }
    unsafe fn bunch_release(&self, bunch: *mut FInBunch) -> RuntimeResult<()> {
        unsafe { crate::net_bunch::release_core_buffer(ptr::addr_of_mut!((*bunch).base.buffer)) }
            .map_err(|_| RuntimeError::new("Failed to release canonical incoming bunch"))
    }
    unsafe fn bunch_resize_buffer(&self, bunch: *mut FInBunch, bytes: i32) -> RuntimeResult<()> {
        unsafe {
            array_storage::set_exact(
                array_slots(ptr::addr_of_mut!((*bunch).base.buffer)),
                bytes,
                1,
                ARRAY_TAG.as_ptr().cast(),
                array_storage::system_allocator(),
            )
        }
        .map_err(|_| RuntimeError::new("Failed to resize canonical incoming bunch"))
    }
    unsafe fn open_channel_count(&self, conn: *mut c_void) -> i32 {
        unsafe { (*conn.cast::<UNetConnection>()).open_channels.num }
    }
    unsafe fn open_channel_at(&self, conn: *mut c_void, index: i32) -> RuntimeResult<*mut UChannel> {
        let channels = unsafe { &(*conn.cast::<UNetConnection>()).open_channels };
        if index < 0 || index >= channels.num || channels.data.is_null() {
            return Err(RuntimeError::new("OpenChannels index is invalid"));
        }
        Ok(unsafe { channels.data.cast::<*mut c_void>().add(index as usize).read_unaligned().cast() })
    }
    unsafe fn known_channel_type(&self, channel_type: i32) -> RuntimeResult<bool> {
        let class_name = match channel_type {
            crate::level_net::CHTYPE_CONTROL => "Engine.ControlChannel",
            crate::level_net::CHTYPE_ACTOR => "Engine.ActorChannel",
            crate::level_net::CHTYPE_FILE => "Engine.FileChannel",
            _ => return Ok(false),
        };
        self.access.native_class(class_name).map(|_| true)
    }
    unsafe fn construct_channel(&self, _conn: *mut c_void, channel_type: i32) -> RuntimeResult<*mut UChannel> {
        unsafe { crate::channel_owner::construct_channel(self.access, channel_type) }
    }
    unsafe fn initialize_channel(
        &self,
        channel_type: i32,
        channel: *mut UChannel,
        conn: *mut c_void,
        index: i32,
        opened_locally: i32,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::channel_owner::initialize_channel(
                self.access,
                channel_type,
                channel,
                conn.cast(),
                index,
                opened_locally,
            )
        }
    }
    unsafe fn add_open_channel(&self, conn: *mut c_void, channel: *mut UChannel) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::add_open_channel(conn.cast(), channel) }
    }
    unsafe fn notify_accepting_channel(
        &self,
        conn: *mut c_void,
        channel: *mut UChannel,
    ) -> RuntimeResult<bool> {
        let driver = unsafe { (*conn.cast::<UNetConnection>()).driver.cast::<UNetDriver>() };
        unsafe { crate::level_net_owner::notify_accepting_channel_for_driver(self.access, driver, channel) }
    }
    unsafe fn channel_received_nak(&self, channel: *mut UChannel, packet_id: i32) -> RuntimeResult<()> {
        if unsafe { (*channel).ch_type } == crate::level_net::CHTYPE_ACTOR {
            unsafe { crate::actor_channel_owner::received_nak(self.access, channel.cast(), packet_id) }
        } else {
            unsafe { crate::channel_owner::received_nak(self.access, channel, packet_id) }
        }
    }
    unsafe fn channel_received_acks(&self, channel: *mut UChannel) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::received_acks(self.access, channel) }
    }
    unsafe fn channel_received_raw(&self, channel: *mut UChannel, bunch: *mut FInBunch) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::received_raw(self.access, channel, bunch) }
    }
    unsafe fn with_close_bunch<T>(
        &self,
        channel: *mut UChannel,
        body: impl FnOnce(*mut FOutBunch) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let mut body_error = None;
        let result = unsafe {
            crate::net_bunch::with_out_core(channel, 1, |bunch| match body(bunch) {
                Ok(value) => Ok(value),
                Err(error) => {
                    body_error = Some(error);
                    Err(crate::net_conn_abi::Thrown)
                }
            })
        };
        if let Some(error) = body_error {
            return Err(error);
        }
        result.map_err(|_| RuntimeError::new("Failed to create canonical channel close bunch"))
    }
    unsafe fn channel_send_bunch(&self, channel: *mut UChannel, bunch: *mut FOutBunch) -> RuntimeResult<()> {
        unsafe { crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, 0) }
            .map(|_| ())
    }
    unsafe fn delete_channel(&self, channel: *mut UChannel) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::delete_channel(self.access, channel) }
    }
}

impl ConnectionTickOps for CorePacketOps<'_> {
    fn app_round(&self, value: f32) -> i32 {
        crate::math::app_round(value)
    }

    unsafe fn log_timeout(&self, timeout: f32, elapsed: f32) -> RuntimeResult<()> {
        let category = self.access.intern_str("DevNet", hp2_core::name_owner::FindName::Add)?;
        let message = format!("Connection timed out after {timeout:.6} seconds ({elapsed:.6})")
            .chars()
            .map(u32::from)
            .collect::<Vec<_>>();
        self.access.emit_log(category, &message)
    }

    unsafe fn actor_channel_exists(
        &self,
        connection: *mut UNetConnection,
        actor: *mut c_void,
    ) -> RuntimeResult<bool> {
        Ok(!unsafe { crate::network_owner::actor_channel(connection, actor.cast())? }.is_null())
    }

    unsafe fn set_bad_connection_alert(&self, actor: *mut c_void, value: bool) {
        // SAFETY: the source casts the connection Actor to APlayerPawn before this bit write.
        // `bBadConnectionAlert` follows `bAutoJump` by fifteen bit fields (EngineClasses.h:1847–1862).
        const ALERT_BIT: usize = APlayerPawn::B_AUTO_JUMP_BIT + 15;
        let byte = unsafe { actor.cast::<u8>().add(ALERT_BIT / 8) };
        let mask = 1_u8 << (ALERT_BIT % 8);
        let flags = unsafe { byte.read() };
        unsafe {
            byte.write(if value { flags | mask } else { flags & !mask });
        }
    }

    unsafe fn channel_tick(&self, channel: *mut UChannel) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::tick(self.access, channel) }
    }

    unsafe fn download_tick(&self, download: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::download_owner::tick(self.access, download.cast()) }
    }
}

struct CoreReceiveFileOps<'a> {
    access: CoreAccess<'a>,
}

impl ConnectionReceiveFileOps for CoreReceiveFileOps<'_> {
    type Error = RuntimeError;

    unsafe fn check_fail(&self, expression: &CStr, line: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(&format!(
            "{} (UnConn.cpp:{line})",
            expression.to_string_lossy()
        )))
    }
    unsafe fn package_count(&self, conn: *mut UNetConnection) -> i32 {
        unsafe { (*(*conn).package_map.cast::<hp2_abi::engine::generated::UPackageMap>()).list.num }
    }
    unsafe fn download_info_count(&self, conn: *mut UNetConnection) -> i32 {
        unsafe { (*conn).download_info.num }
    }
    unsafe fn add_default_download_info(&self, conn: *mut UNetConnection) -> RuntimeResult<()> {
        let index = unsafe {
            array_storage::add_zeroed(
                array_slots(ptr::addr_of_mut!((*conn).download_info)),
                1,
                mem::size_of::<FDownloadInfo>() as i32,
                ARRAY_TAG.as_ptr().cast(),
                array_storage::system_allocator(),
            )
        }
        .map_err(|_| RuntimeError::new("Failed to add default download information"))?;
        let info = unsafe { (*conn).download_info.data.cast::<FDownloadInfo>().add(index as usize) };
        let class = self.access.native_class("Engine.ChannelDownload")?;
        unsafe {
            (*info).class = class.cast();
            (*info).compression = 0;
        }
        const CLASS_NAME: [u32; 24] = [
            69, 110, 103, 105, 110, 101, 46, 85, 67, 104, 97, 110, 110, 101, 108,
            68, 111, 119, 110, 108, 111, 97, 100, 0,
        ];
        const EMPTY: [u32; 1] = [0];
        if let Err(error) = unsafe { self.access.assign_fstring(ptr::addr_of_mut!((*info).class_name), &CLASS_NAME) } {
            unsafe {
                let _ = array_storage::remove(
                    array_slots(ptr::addr_of_mut!((*conn).download_info)),
                    index as i32,
                    1,
                    mem::size_of::<FDownloadInfo>() as i32,
                    ARRAY_TAG.as_ptr().cast(),
                    array_storage::system_allocator(),
                );
            }
            return Err(error);
        }
        if let Err(error) = unsafe { self.access.assign_fstring(ptr::addr_of_mut!((*info).params), &EMPTY) } {
            unsafe {
                let _ = self.access.destroy_owned_fstring_array(ptr::addr_of_mut!((*info).class_name));
                let _ = array_storage::remove(
                    array_slots(ptr::addr_of_mut!((*conn).download_info)),
                    index as i32,
                    1,
                    mem::size_of::<FDownloadInfo>() as i32,
                    ARRAY_TAG.as_ptr().cast(),
                    array_storage::system_allocator(),
                );
            }
            return Err(error);
        }
        Ok(())
    }
    unsafe fn delete_download(&self, download: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.access.conditional_destroy(download.cast())?; }
        unsafe { self.access.delete_object(download.cast()) }
    }
    unsafe fn construct_download(&self, conn: *mut UNetConnection) -> RuntimeResult<*mut c_void> {
        unsafe {
            crate::download_owner::construct_from_first_method(self.access, conn)
                .map(|download| download.cast())
        }
    }
    unsafe fn receive_file(
        &self,
        conn: *mut UNetConnection,
        package_index: i32,
        params: *const u32,
        compression: i32,
    ) -> RuntimeResult<()> {
        unsafe {
            crate::download_owner::receive_file_dispatch(
                self.access,
                (*conn).download.cast(),
                conn,
                package_index,
                params,
                compression != 0,
            )
        }
    }
}

unsafe fn array_slots(array: *mut hp2_abi::engine::generated::TArrayRaw) -> array_storage::Hp2ArraySlots {
    unsafe {
        array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data).cast(),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}
unsafe fn release_array(array: *mut TArrayRaw) -> RuntimeResult<()> {
    unsafe { crate::net_bunch::release_core_buffer(array) }
        .map_err(|_| RuntimeError::new("Failed to release connection array storage"))
}


const _: () = assert!(mem::size_of::<UNetConnection>() == 20_344);
