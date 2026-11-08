//! Rust port of `UnNetDrv.cpp`: `UPackageMapLevel` object serialization and the
//! `UNetDriver` construction, tick, serialize, teardown, exec, and actor
//! notification bodies.
//!
//! C++ keeps constructors, virtual tables, reflected property objects, archive
//! and log leaves, and the object model. Those are reached through
//! [`Hp2NetDriverServices`]. Every `S` leaf returns a `Hp2FfiCall` status: zero
//! succeeds, and nonzero means a C++ exception was captured, so Rust stops
//! before any later leaf runs. `K` leaves are non-throwing accessors and return
//! their value directly.

use crate::net_driver_abi::Hp2NetDriverServices;
use hp2_abi::engine::generated::{
    AActor, FArchive, UActorChannel, UArrayProperty, UChannel, UNetConnection, UNetDriver,
    UObject, UPackageMapLevel,
};
use std::ffi::c_void;
use std::mem::offset_of;
use std::ptr::null_mut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrown;

const RF_PUBLIC: u32 = 0x4;
const CPF_CONFIG: u32 = 0x4000;
const MAX_CHANNELS: i32 = 1023;
const CHANNEL_TYPE_ACTOR: i32 = 2;
const USOCK_CLOSED: i32 = 1;
const INDEX_NONE: u32 = u32::MAX;
const DEFAULT_MAX_CLIENT_RATE: i32 = 25000;

/// Builds a NUL-terminated `TCHAR` (`wchar_t`) string. `N` must equal the text
/// length plus one, which is checked at compile time.
const fn wide<const N: usize>(text: &str) -> [u32; N] {
    let bytes = text.as_bytes();
    assert!(
        bytes.len() + 1 == N,
        "wide: array length must be text length plus NUL"
    );
    let mut out = [0_u32; N];
    let mut i = 0;
    while i < bytes.len() {
        out[i] = bytes[i] as u32;
        i += 1;
    }
    out
}

static PROFILE_STATS: [u32; 13] = wide("profilestats");
static SOCKETS: [u32; 8] = wide("SOCKETS");
static CONNECTIONS: [u32; 13] = wide("Connections:");
static ROLE: [u32; 5] = wide("Role");
static REMOTE_ROLE: [u32; 11] = wide("RemoteRole");
static CLIENT: [u32; 7] = wide("Client");
static STR_PROPERTY0: [u32; 13] = wide("StrProperty0");
static CONNECTION_TIMEOUT: [u32; 18] = wide("ConnectionTimeout");
static INITIAL_CONNECT_TIMEOUT: [u32; 22] = wide("InitialConnectTimeout");
static KEEP_ALIVE_TIME: [u32; 14] = wide("KeepAliveTime");
static RELEVANT_TIMEOUT: [u32; 16] = wide("RelevantTimeout");
static SPAWN_PRIORITY_SECONDS: [u32; 21] = wide("SpawnPrioritySeconds");
static SERVER_TRAVEL_PAUSE: [u32; 18] = wide("ServerTravelPause");
static MAX_CLIENT_RATE: [u32; 14] = wide("MaxClientRate");
static NET_SERVER_MAX_TICK_RATE: [u32; 21] = wide("NetServerMaxTickRate");
static LAN_SERVER_MAX_TICK_RATE: [u32; 21] = wide("LanServerMaxTickRate");
static ALLOW_DOWNLOADS: [u32; 15] = wide("AllowDownloads");
static MAX_DOWNLOAD_SIZE: [u32; 16] = wide("MaxDownloadSize");
static DOWNLOAD_MANAGERS: [u32; 17] = wide("DownloadManagers");

#[inline]
fn check(status: i32) -> Result<(), Thrown> {
    if status == 0 { Ok(()) } else { Err(Thrown) }
}

/// `UPackageMapLevel::CanSerializeObject`. A non-static, non-`bNoDelete` actor
/// serializes only while this map's connection holds an actor channel for it;
/// everything else serializes unconditionally.
///
/// # Safety
/// `map` must be a live `UPackageMapLevel`; `object` must be a live `UObject`
/// or null.
pub unsafe fn can_serialize_object(
    svc: &Hp2NetDriverServices,
    map: *mut UPackageMapLevel,
    object: *mut c_void,
) -> bool {
    unsafe {
        let actor = (svc.drv_cast_actor)(object);
        if !actor.is_null()
            && !(*actor.cast::<AActor>()).b_static()
            && !(*actor.cast::<AActor>()).b_no_delete()
        {
            !(svc.drv_conn_find_actor_channel)((*map).connection, actor).is_null()
        } else {
            true
        }
    }
}

/// `UPackageMapLevel::SerializeObject`.
///
/// Loading reads the dynamic-or-static selector, then resolves the object
/// through the connection's channel table or the package map, and rejects an
/// object whose class does not match. Saving writes the same selector and index
/// for dynamic actors, or the package-map index for everything else.
///
/// # Safety
/// `map`, `archive`, and `class` must be live C++ objects; `object` must point
/// to a writable `UObject*`.
pub unsafe fn serialize_object(
    svc: &Hp2NetDriverServices,
    map: *mut UPackageMapLevel,
    archive: *mut c_void,
    class: *mut c_void,
    object: *mut *mut c_void,
) -> Result<bool, Thrown> {
    unsafe {
        let map_ptr = map.cast::<c_void>();
        let connection = (*map).connection;

        if (*archive.cast::<FArchive>()).ar_is_loading != 0 {
            *object = null_mut();
            let mut index = 0_u32;
            let mut dynamic = 0_u8;
            check((svc.drv_ar_serialize_bits)(archive, &mut dynamic, 1))?;
            if dynamic != 0 {
                check((svc.drv_ar_serialize_int)(
                    archive,
                    &mut index,
                    MAX_CHANNELS,
                ))?;
                if index != 0 && (*archive.cast::<FArchive>()).ar_is_error == 0 && index < MAX_CHANNELS as u32
                {
                    let channel = (*connection.cast::<UNetConnection>()).channels[index as usize];
                    if !channel.is_null() {
                        let ch = channel.cast::<UChannel>();
                        if (*ch).ch_type == CHANNEL_TYPE_ACTOR && (*ch).closing == 0 {
                        *object = (*channel.cast::<UActorChannel>()).actor;
                        }
                    }
                }
            } else {
                let max_index = (svc.drv_max_object_index)(map_ptr);
                check((svc.drv_ar_serialize_int)(archive, &mut index, max_index))?;
                if (*archive.cast::<FArchive>()).ar_is_error == 0 {
                    let mut mapped = null_mut();
                    check((svc.drv_index_to_object)(
                        map_ptr,
                        index as i32,
                        1,
                        &mut mapped,
                    ))?;
                    *object = mapped;
                }
            }

            if !(*object).is_null() && (svc.drv_object_is_a)(*object, class) == 0 {
                check((svc.drv_log_forged_object)(*object, class))?;
                *object = null_mut();
            }
            return Ok(true);
        }

        let actor = (svc.drv_cast_actor)(*object);
        if !actor.is_null()
            && !(*actor.cast::<AActor>()).b_static()
            && !(*actor.cast::<AActor>()).b_no_delete()
        {
            let mut dynamic = 1_u8;
            check((svc.drv_ar_serialize_bits)(archive, &mut dynamic, 1))?;
            let channel = (svc.drv_conn_find_actor_channel)(connection, actor);
            let mut index = 0_u32;
            let mut mapped = false;
            if !channel.is_null() {
                let ch = channel.cast::<UChannel>();
                index = (*ch).ch_index as u32;
                mapped = (*ch).open_acked != 0;
            }
            check((svc.drv_ar_serialize_int)(
                archive,
                &mut index,
                MAX_CHANNELS,
            ))?;
            return Ok(mapped);
        }

        let mut index = 0_u32;
        if !(*object).is_null() {
            let mut raw = 0_i32;
            check((svc.drv_object_to_index)(map_ptr, *object, &mut raw))?;
            index = raw as u32;
        }
        if (*object).is_null() || index == INDEX_NONE {
            let mut dynamic = 1_u8;
            check((svc.drv_ar_serialize_bits)(archive, &mut dynamic, 1))?;
            check((svc.drv_ar_serialize_int)(
                archive,
                &mut index,
                MAX_CHANNELS,
            ))?;
        } else {
            let mut dynamic = 0_u8;
            check((svc.drv_ar_serialize_bits)(archive, &mut dynamic, 1))?;
            let max_index = (svc.drv_max_object_index)(map_ptr);
            check((svc.drv_ar_serialize_int)(archive, &mut index, max_index))?;
        }
        Ok(true)
    }
}

pub(crate) trait DriverLifetimeOps {
    type Error;
    fn find_actor_property(&mut self, remote: bool) -> Result<*mut c_void, Self::Error>;
    fn new_package_map(&mut self) -> Result<*mut c_void, Self::Error>;
    fn profile_stats(&mut self) -> i32;
    fn delete_connection(&mut self, connection: *mut c_void) -> Result<(), Self::Error>;
    fn low_level_destroy(&mut self, driver: *mut UNetDriver) -> Result<(), Self::Error>;
    fn delete_package_map(&mut self, map: *mut c_void) -> Result<(), Self::Error>;
    fn destroy_object(&mut self, driver: *mut UNetDriver) -> Result<(), Self::Error>;
}

struct NativeDriverOps<'a>(&'a Hp2NetDriverServices);

impl DriverLifetimeOps for NativeDriverOps<'_> {
    type Error = Thrown;
    fn find_actor_property(&mut self, remote: bool) -> Result<*mut c_void, Thrown> {
        let mut property = null_mut();
        let name = if remote { REMOTE_ROLE.as_ptr() } else { ROLE.as_ptr() };
        unsafe { check((self.0.drv_find_actor_property)(name, &mut property))?; }
        Ok(property)
    }
    fn new_package_map(&mut self) -> Result<*mut c_void, Thrown> {
        let mut map = null_mut();
        unsafe { check((self.0.drv_new_package_map)(&mut map))?; }
        Ok(map)
    }
    fn profile_stats(&mut self) -> i32 {
        unsafe { (self.0.drv_parse_param)((self.0.drv_app_cmdline)(), PROFILE_STATS.as_ptr()) }
    }
    fn delete_connection(&mut self, connection: *mut c_void) -> Result<(), Thrown> {
        unsafe { check((self.0.drv_delete_connection)(connection)) }
    }
    fn low_level_destroy(&mut self, driver: *mut UNetDriver) -> Result<(), Thrown> {
        unsafe { check((self.0.drv_low_level_destroy)(driver.cast())) }
    }
    fn delete_package_map(&mut self, map: *mut c_void) -> Result<(), Thrown> {
        unsafe { check((self.0.drv_delete_package_map)(map)) }
    }
    fn destroy_object(&mut self, driver: *mut UNetDriver) -> Result<(), Thrown> {
        unsafe { check((self.0.drv_super_destroy)(driver.cast())) }
    }
}

pub(crate) unsafe fn construct_with_ops<O: DriverLifetimeOps>(
    driver: *mut UNetDriver, ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        (*driver).role_property = ops.find_actor_property(false)?;
        (*driver).remote_role_property = ops.find_actor_property(true)?;
        (*driver).master_map = ops.new_package_map()?;
        (*driver).profile_stats = ops.profile_stats();
    }
    Ok(())
}

pub(crate) unsafe fn destroy_with_ops<O: DriverLifetimeOps>(
    driver: *mut UNetDriver, ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        let server = (*driver).server_connection;
        if !server.is_null() { ops.delete_connection(server)?; }
        while (*driver).client_connections.num != 0 {
            let connection = (*driver).client_connections.data.cast::<*mut c_void>().read_unaligned();
            ops.delete_connection(connection)?;
        }
        ops.low_level_destroy(driver)?;
        ops.delete_package_map((*driver).master_map)?;
        ops.destroy_object(driver)?;
    }
    Ok(())
}

/// `UNetDriver::UNetDriver` body. The C++ shell keeps the member initializers
/// for `ClientConnections`, `Time`, and `DownloadManagers`.
///
/// # Safety
/// `driver` must be a live, partially constructed `UNetDriver`.
pub unsafe fn construct(svc: &Hp2NetDriverServices, driver: *mut UNetDriver) -> Result<(), Thrown> {
    unsafe { construct_with_ops(driver, &mut NativeDriverOps(svc)) }
}

/// `UNetDriver::StaticConstructor`: registers the `CPF_Config` reflected
/// properties on the class default object, then sets `MaxClientRate`.
///
/// # Safety
/// `driver` must be the live class default object of `UNetDriver`.
pub unsafe fn static_constructor(
    svc: &Hp2NetDriverServices,
    driver: *mut UNetDriver,
) -> Result<(), Thrown> {
    unsafe {
        let outer = (*driver.cast::<UObject>()).class;
        let mut prop: *mut c_void = null_mut();

        macro_rules! prop {
            ($call:ident, $name:ident, $field:ident) => {
                check((svc.$call)(
                    outer,
                    $name.as_ptr(),
                    RF_PUBLIC,
                    offset_of!(UNetDriver, $field) as i32,
                    CLIENT.as_ptr(),
                    CPF_CONFIG,
                    &mut prop,
                ))?
            };
        }

        prop!(drv_static_float, CONNECTION_TIMEOUT, connection_timeout);
        prop!(
            drv_static_float,
            INITIAL_CONNECT_TIMEOUT,
            initial_connect_timeout
        );
        prop!(drv_static_float, KEEP_ALIVE_TIME, keep_alive_time);
        prop!(drv_static_float, RELEVANT_TIMEOUT, relevant_timeout);
        prop!(
            drv_static_float,
            SPAWN_PRIORITY_SECONDS,
            spawn_priority_seconds
        );
        prop!(drv_static_float, SERVER_TRAVEL_PAUSE, server_travel_pause);
        prop!(drv_static_int, MAX_CLIENT_RATE, max_client_rate);
        prop!(
            drv_static_int,
            NET_SERVER_MAX_TICK_RATE,
            net_server_max_tick_rate
        );
        prop!(
            drv_static_int,
            LAN_SERVER_MAX_TICK_RATE,
            lan_server_max_tick_rate
        );
        prop!(drv_static_bool, ALLOW_DOWNLOADS, allow_downloads);
        prop!(drv_static_int, MAX_DOWNLOAD_SIZE, max_download_size);

        let mut array: *mut c_void = null_mut();
        check((svc.drv_static_array)(
            outer,
            DOWNLOAD_MANAGERS.as_ptr(),
            RF_PUBLIC,
            offset_of!(UNetDriver, download_managers) as i32,
            CLIENT.as_ptr(),
            CPF_CONFIG,
            &mut array,
        ))?;
        let mut inner: *mut c_void = null_mut();
        check((svc.drv_static_str)(
            array,
            STR_PROPERTY0.as_ptr(),
            RF_PUBLIC,
            &mut inner,
        ))?;
        (*array.cast::<UArrayProperty>()).inner = inner;

        (*driver).max_client_rate = DEFAULT_MAX_CLIENT_RATE;
    }
    Ok(())
}

/// `UNetDriver::TickFlush`: tick the server connection, then every client
/// connection. The client count is re-read on each loop test.
///
/// # Safety
/// `driver` and its connection pointers must be live engine objects.
pub unsafe fn tick_flush(
    svc: &Hp2NetDriverServices,
    driver: *mut UNetDriver,
) -> Result<(), Thrown> {
    unsafe {
        tick_flush_with(driver, |connection| unsafe {
            check((svc.drv_conn_tick)(connection))
        })
    }
}

/// Shared connection iteration for native and canonical driver TickFlush paths.
///
/// # Safety
/// `driver` and every connection passed to `tick_connection` must remain live.
pub(crate) unsafe fn tick_flush_with<E>(
    driver: *mut UNetDriver,
    mut tick_connection: impl FnMut(*mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        let server = (*driver).server_connection;
        if !server.is_null() {
            tick_connection(server)?;
        }
        let mut index = 0_i32;
        while index < (*driver).client_connections.num {
            let connections = (*driver).client_connections.data.cast::<*mut c_void>();
            tick_connection(*connections.add(index as usize))?;
            index += 1;
        }
    }
    Ok(())
}

/// `UNetDriver::InitConnect` and `UNetDriver::InitListen`: retain the network
/// notify object and succeed.
///
/// # Safety
/// `driver` must point to a live `UNetDriver`; `notify` is stored unchecked.
pub unsafe fn init_notify(driver: *mut UNetDriver, notify: *mut c_void) -> bool {
    unsafe { (*driver).notify = notify };
    true
}

/// `UNetDriver::TickDispatch`: reset cycle counters, advance time, and when
/// there is no server connection delete closed client connections, counting
/// down so deletions do not skip entries.
///
/// # Safety
/// `driver` and each client connection must be live engine objects.
pub unsafe fn tick_dispatch(
    svc: &Hp2NetDriverServices,
    driver: *mut UNetDriver,
    delta_time: f32,
) -> Result<(), Thrown> {
    unsafe {
        tick_dispatch_with(driver, delta_time, &mut |connection| {
            check((svc.drv_delete_connection)(connection))
        })
    }
}

/// Shared dispatch bookkeeping for native and canonical drivers.
///
/// # Safety
/// The driver and reached connections remain live. Deleting a connection must
/// remove it from its driver's client array, as the source destructor does.
pub(crate) unsafe fn tick_dispatch_with<E>(
    driver: *mut UNetDriver,
    delta_time: f32,
    delete_connection: &mut impl FnMut(*mut c_void) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        (*driver).send_cycles = 0;
        (*driver).recv_cycles = 0;

        // FTime::operator+= on the fixed-point time word (FIXTIME = 2^32).
        let time = i64::from_ne_bytes((*driver).time);
        let increment = (delta_time * 4_294_967_296.0_f32) as i64;
        (*driver).time = time.wrapping_add(increment).to_ne_bytes();

        if (*driver).server_connection.is_null() {
            let mut i = (*driver).client_connections.num - 1;
            while i >= 0 {
                // Deleting an item may reallocate the TArray; re-read Data each pass.
                let data = (*driver).client_connections.data.cast::<*mut c_void>();
                let connection = *data.add(i as usize);
                if (*connection.cast::<UNetConnection>()).state == USOCK_CLOSED {
                    delete_connection(connection)?;
                }
                i -= 1;
            }
        }
    }
    Ok(())
}

/// `UNetDriver::Serialize`: `Super::Serialize`, then the five reference
/// archive operators in the oracle's order.
///
/// # Safety
/// `driver` and `archive` must be live C++ objects.
pub unsafe fn serialize(
    svc: &Hp2NetDriverServices,
    driver: *mut UNetDriver,
    archive: *mut c_void,
) -> Result<(), Thrown> {
    unsafe {
        check((svc.drv_super_serialize)(driver.cast(), archive))?;
        check((svc.drv_serialize_references)(archive, driver.cast()))?;
    }
    Ok(())
}

/// `UNetDriver::Destroy`: delete the server connection, delete every client
/// connection, then low-level destroy, delete the master map, and run the
/// `UObject` base destructor. The client loop re-reads `Num()` each pass, since
/// deleting a connection removes it from the driver array.
///
/// # Safety
/// `driver` must be a live `UNetDriver` owned by the caller being destroyed.
pub unsafe fn destroy(svc: &Hp2NetDriverServices, driver: *mut UNetDriver) -> Result<(), Thrown> {
    unsafe { destroy_with_ops(driver, &mut NativeDriverOps(svc)) }
}

pub(crate) trait SocketExecOps {
    type Error;
    unsafe fn matches(&mut self, command: *const u32) -> Result<bool, Self::Error>;
    fn log_heading(&mut self) -> Result<(), Self::Error>;
    unsafe fn log_connection(&mut self, connection: *mut UNetConnection, server: bool) -> Result<(), Self::Error>;
    unsafe fn log_channel(&mut self, channel: *mut UChannel) -> Result<(), Self::Error>;
}

struct NativeSocketExecOps<'a> {
    services: &'a Hp2NetDriverServices,
    output: *mut c_void,
}

impl SocketExecOps for NativeSocketExecOps<'_> {
    type Error = Thrown;
    unsafe fn matches(&mut self, mut command: *const u32) -> Result<bool, Thrown> {
        Ok(unsafe { (self.services.drv_parse_command)(&mut command, SOCKETS.as_ptr()) != 0 })
    }
    fn log_heading(&mut self) -> Result<(), Thrown> {
        unsafe { check((self.services.drv_exec_logf)(self.output, CONNECTIONS.as_ptr())) }
    }
    unsafe fn log_connection(&mut self, connection: *mut UNetConnection, server: bool) -> Result<(), Thrown> {
        unsafe {
            if server { check((self.services.drv_exec_log_server)(self.output, connection.cast())) }
            else { check((self.services.drv_exec_log_client)(self.output, connection.cast())) }
        }
    }
    unsafe fn log_channel(&mut self, channel: *mut UChannel) -> Result<(), Thrown> {
        unsafe { check((self.services.drv_exec_log_channel)(self.output, channel.cast())) }
    }
}

unsafe fn log_channels_with<O: SocketExecOps>(
    ops: &mut O, connection: *mut UNetConnection,
) -> Result<(), O::Error> {
    unsafe {
        let mut index = 0_i32;
        while index < (*connection).open_channels.num {
            let data = (*connection).open_channels.data.cast::<*mut UChannel>();
            ops.log_channel(*data.add(index as usize))?;
            index += 1;
        }
    }
    Ok(())
}

/// Shared source SOCKETS command, including dynamic connection/channel descriptions.
///
/// # Safety
/// Driver, command, and every reached connection/channel remain live.
pub(crate) unsafe fn exec_sockets_with<O: SocketExecOps>(
    ops: &mut O, driver: *mut UNetDriver, command: *const u32,
) -> Result<bool, O::Error> {
    unsafe {
        if !ops.matches(command)? { return Ok(false); }
        ops.log_heading()?;
        let server = (*driver).server_connection.cast::<UNetConnection>();
        if !server.is_null() {
            ops.log_connection(server, true)?;
            log_channels_with(ops, server)?;
        }
        let mut index = 0_i32;
        while index < (*driver).client_connections.num {
            let data = (*driver).client_connections.data.cast::<*mut UNetConnection>();
            let connection = *data.add(index as usize);
            ops.log_connection(connection, false)?;
            log_channels_with(ops, connection)?;
            index += 1;
        }
    }
    Ok(true)
}

/// # Safety
/// The driver and its graph, native output device, command and services are live.
pub unsafe fn exec_sockets(
    svc: &Hp2NetDriverServices, driver: *mut UNetDriver,
    command: *const u32, output: *mut c_void,
) -> Result<bool, Thrown> {
    unsafe { exec_sockets_with(&mut NativeSocketExecOps { services: svc, output }, driver, command) }
}

pub(crate) trait DriverActorOps {
    type Error;
    fn remove_sent_temporary(&mut self, connection: *mut c_void, actor: *mut c_void) -> Result<(), Self::Error>;
    fn actor_channel(&mut self, connection: *mut c_void, actor: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn close_locally_opened_channel(&mut self, channel: *mut c_void) -> Result<(), Self::Error>;
}

impl DriverActorOps for NativeDriverOps<'_> {
    type Error = Thrown;
    fn remove_sent_temporary(&mut self, connection: *mut c_void, actor: *mut c_void) -> Result<(), Thrown> {
        unsafe { check((self.0.drv_conn_remove_sent_temporary)(connection, actor)) }
    }
    fn actor_channel(&mut self, connection: *mut c_void, actor: *mut c_void) -> Result<*mut c_void, Thrown> {
        Ok(unsafe { (self.0.drv_conn_find_actor_channel)(connection, actor) })
    }
    fn close_locally_opened_channel(&mut self, channel: *mut c_void) -> Result<(), Thrown> {
        unsafe { check((self.0.drv_channel_close_checked)(channel)) }
    }
}

pub(crate) unsafe fn notify_actor_destroyed_with_ops<O: DriverActorOps>(
    driver: *mut UNetDriver, actor: *mut c_void, ops: &mut O,
) -> Result<(), O::Error> {
    unsafe {
        let mut index = (*driver).client_connections.num - 1;
        while index >= 0 {
            let connection = (*driver).client_connections.data.cast::<*mut c_void>()
                .add(index as usize).read_unaligned();
            if (*actor.cast::<AActor>()).b_net_temporary() {
                ops.remove_sent_temporary(connection, actor)?;
            }
            let channel = ops.actor_channel(connection, actor)?;
            if !channel.is_null() { ops.close_locally_opened_channel(channel)?; }
            index -= 1;
        }
    }
    Ok(())
}

/// `UNetDriver::NotifyActorDestroyed`: counting down through the clients, drop a
/// temporary actor from each connection's sent list, then close any actor
/// channel that is open for it.
///
/// # Safety
/// `driver` and each client connection must be live; `actor` must be a live
/// `AActor`.
pub unsafe fn notify_actor_destroyed(
    svc: &Hp2NetDriverServices,
    driver: *mut UNetDriver,
    actor: *mut c_void,
) -> Result<(), Thrown> {
    unsafe { notify_actor_destroyed_with_ops(driver, actor, &mut NativeDriverOps(svc)) }
}
