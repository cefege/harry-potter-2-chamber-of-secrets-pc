//! Canonical network-driver, package-map and pending-level ownership.

use hp2_abi::engine::generated::{AActor, FActorChannelMapPair, FURL, TArrayRaw, UChannel,
    UDemoPlayPendingLevel, UGameEngine, ULevelBase, UNetConnection, UNetDriver,
    UNetPendingLevel, UObject, UPackageMapLevel, UPendingLevel};
use hp2_core::{array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupDefault, StartupProperty, StartupPropertyKind},
    name_owner::FindName, object_archive::{ArchiveMode, ObjectArchive}, object_model,
    object_owner::ClassOps};
use std::{alloc::Layout, ffi::c_void, mem, ptr};

/// Constructs the canonical pending client graph and performs NetPendingLevel's
/// configured driver connect/HELLO sequence.
///
/// # Safety
/// `engine` is a live GameEngine and `url` is a live initialized FURL. Returned
/// pending storage is owned by the engine and remains valid until deletion.
pub unsafe fn new_pending(
    access: CoreAccess<'_>,
    engine: *mut UGameEngine,
    url: *const hp2_abi::engine::generated::FURL,
) -> RuntimeResult<*mut UNetPendingLevel> {
    if engine.is_null() || url.is_null() {
        return Err(RuntimeError::new("Invalid NetPendingLevel constructor arguments"));
    }
    let class = access.native_class("Engine.NetPendingLevel")?;
    let pending = unsafe {
        access.construct_object(
            class,
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )?
    }.cast::<UNetPendingLevel>();
    let base = unsafe { ptr::addr_of_mut!((*pending).base) };
    unsafe {
        (*base).engine = engine.cast();
        if let Err(error) = crate::url_owner::UrlOwner::copy_construct(
            access,
            ptr::addr_of_mut!((*base).url),
            url,
        ) {
            let _ = access.delete_object(pending.cast());
            return Err(error);
        }
    }

    // The configured class is dynamic; Core dispatch is explicit and accepts
    // only classes whose complete InitConnect implementation is source-backed.
    let base_class = match access.native_class("Engine.NetDriver") {
        Ok(class) => class,
        Err(error) => {
            unsafe { let _ = access.delete_object(pending.cast()); }
            return Err(error);
        }
    };
    let driver_class = match unsafe {
        access.static_load_class(
            base_class,
            ptr::null_mut(),
            hp2_core::lit!("ini:Engine.Engine.NetworkDevice"),
            2,
        )
    } {
        Ok(class) => class,
        Err(error) => {
            unsafe { let _ = access.delete_object(pending.cast()); }
            return Err(error);
        }
    };
    let driver = match unsafe {
        access.construct_object(
            driver_class,
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )
    } {
        Ok(driver) => driver.cast::<UNetDriver>(),
        Err(error) => {
            unsafe { access.delete_object(pending.cast())?; }
            return Err(error);
        }
    };
    unsafe { (*base).net_driver = driver.cast(); }
    let base_class = access.native_class("Engine.NetDriver")?;
    if unsafe { (*driver.cast::<UObject>()).class == base_class.cast() } {
        unsafe {
            access.delete_object(driver.cast())?;
            (*base).net_driver = ptr::null_mut();
            access.delete_object(pending.cast())?;
        }
        return Err(RuntimeError::new(
            "Configured Engine.NetDriver has no client InitConnect transport",
        ));
    }
    unsafe {
        access.delete_object(driver.cast())?;
        (*base).net_driver = ptr::null_mut();
        access.delete_object(pending.cast())?;
    }
    Err(RuntimeError::new(
        "Configured NetworkDevice has no canonical Core InitConnect transport",
    ))
}

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const SUBSYSTEM: ClassOps = NativeRecordKind::Subsystem.operations();

const fn config(name: &'static str, kind: StartupPropertyKind, offset: usize) -> StartupProperty {
    StartupProperty { name, kind, category: "Client", property_flags: 0x0000_4000,
        object_flags: 4, inner_name: None, offset, array_dim: 1 }
}
const DRIVER_PROPERTIES: &[StartupProperty] = &[
    config("ConnectionTimeout", StartupPropertyKind::Float, mem::offset_of!(UNetDriver, connection_timeout)),
    config("InitialConnectTimeout", StartupPropertyKind::Float, mem::offset_of!(UNetDriver, initial_connect_timeout)),
    config("KeepAliveTime", StartupPropertyKind::Float, mem::offset_of!(UNetDriver, keep_alive_time)),
    config("RelevantTimeout", StartupPropertyKind::Float, mem::offset_of!(UNetDriver, relevant_timeout)),
    config("SpawnPrioritySeconds", StartupPropertyKind::Float, mem::offset_of!(UNetDriver, spawn_priority_seconds)),
    config("ServerTravelPause", StartupPropertyKind::Float, mem::offset_of!(UNetDriver, server_travel_pause)),
    config("MaxClientRate", StartupPropertyKind::Int, mem::offset_of!(UNetDriver, max_client_rate)),
    config("NetServerMaxTickRate", StartupPropertyKind::Int, mem::offset_of!(UNetDriver, net_server_max_tick_rate)),
    config("LanServerMaxTickRate", StartupPropertyKind::Int, mem::offset_of!(UNetDriver, lan_server_max_tick_rate)),
    config("AllowDownloads", StartupPropertyKind::Bool { bit_mask: 1 }, mem::offset_of!(UNetDriver, allow_downloads)),
    config("MaxDownloadSize", StartupPropertyKind::Int, mem::offset_of!(UNetDriver, max_download_size)),
    StartupProperty { inner_name: Some("StrProperty0"),
        ..config("DownloadManagers", StartupPropertyKind::ArrayOfString, mem::offset_of!(UNetDriver, download_managers)) },
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec { short_name: "PackageMapLevel", package: "Engine", super_name: Some("PackageMap"),
        within: "Object", flags: 8, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: NativeRecordKind::PackageMap, layout: Some(Layout::new::<UPackageMapLevel>()),
        operations: None, properties: &[], defaults: &[] },
    NativeClassSpec { short_name: "NetDriver", package: "Engine", super_name: Some("Subsystem"),
        within: "Object", flags: 1 | 4 | 8, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object, layout: Some(Layout::new::<UNetDriver>()),
        operations: Some(DRIVER_OPERATIONS), properties: DRIVER_PROPERTIES,
        defaults: &[StartupDefault::SetInt { field: "MaxClientRate", value: 25000 }] },
    // DECLARE_ABSTRACT_CLASS names UObject as the reflected superclass, although
    // the C++ physical base and inherited virtual lifecycle are ULevelBase.
    NativeClassSpec { short_name: "PendingLevel", package: "Engine", super_name: Some("Object"),
        within: "Object", flags: 1, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object, layout: Some(Layout::new::<UPendingLevel>()),
        operations: Some(PENDING_OPERATIONS), properties: &[], defaults: &[] },
    NativeClassSpec { short_name: "NetPendingLevel", package: "Engine", super_name: Some("PendingLevel"),
        within: "Object", flags: 8, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object, layout: Some(Layout::new::<UNetPendingLevel>()),
        operations: Some(PENDING_OPERATIONS), properties: &[], defaults: &[] },
    NativeClassSpec { short_name: "DemoPlayPendingLevel", package: "Engine", super_name: Some("PendingLevel"),
        within: "Object", flags: 8, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: NativeRecordKind::Object, layout: Some(Layout::new::<UDemoPlayPendingLevel>()),
        operations: Some(PENDING_OPERATIONS), properties: &[], defaults: &[] },
];

pub(crate) const DRIVER_OPERATIONS: ClassOps = {
    let mut operations = SUBSYSTEM;
    operations.construct = construct_driver;
    operations.destroy = destroy_driver;
    operations.finalize_members = finalize_driver_members;
    operations.serialize = serialize_driver;
    operations.visit_references = serialize_driver;
    operations
};
const PENDING_OPERATIONS: ClassOps = {
    let mut operations = crate::level_owner::LEVEL_BASE_OPERATIONS;
    operations.construct = construct_pending;
    operations.destroy = destroy_pending;
    operations.finalize_members = finalize_pending;
    operations
};

unsafe fn construct_pending(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::level_owner::LEVEL_BASE_OPERATIONS.construct)(access, object)?; }
    let pending = object.cast::<UPendingLevel>();
    unsafe {
        ptr::write_bytes(
            object.cast::<u8>().add(std::mem::size_of::<ULevelBase>()),
            0,
            std::mem::size_of::<UPendingLevel>() - std::mem::size_of::<ULevelBase>(),
        );
    }
    for string in unsafe {
        [
            ptr::addr_of_mut!((*pending).error),
            ptr::addr_of_mut!((*pending).fail_code),
            ptr::addr_of_mut!((*pending).fail_url),
        ]
    } {
        if let Err(error) = unsafe { access.assign_fstring(string, &[0]) } {
            return Err(error);
        }
    }
    unsafe { (*pending).actors.owner = object.cast(); }
    Ok(())
}

fn slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe { Hp2ArraySlots { data: ptr::addr_of_mut!((*array).data),
        num: ptr::addr_of_mut!((*array).num), max: ptr::addr_of_mut!((*array).max) } }
}
fn array_error(code: i32) -> RuntimeError {
    RuntimeError::new("Canonical network array operation failed").with_context(&code.to_string())
}

struct DriverOps<'a>(CoreAccess<'a>);
impl crate::net_driver::DriverLifetimeOps for DriverOps<'_> {
    type Error = RuntimeError;
    fn find_actor_property(&mut self, remote: bool) -> RuntimeResult<*mut c_void> {
        let actor = self.0.native_class("Engine.Actor")?;
        let property = self.0.native_class("Property")?;
        let name = self.0.intern_str(if remote { "RemoteRole" } else { "Role" }, FindName::Add)?;
        // FindField legitimately returns null before the script package supplies Role.
        Ok(unsafe { self.0.find_field(actor.cast(), name, property).cast() })
    }
    fn new_package_map(&mut self) -> RuntimeResult<*mut c_void> {
        let class = self.0.native_class("Engine.PackageMapLevel")?;
        Ok(unsafe { self.0.construct_object(class, self.0.transient_package(), 0, 0, ptr::null_mut())?.cast() })
    }
    fn profile_stats(&mut self) -> i32 {
        const PARAM: [i32; 13] = [112,114,111,102,105,108,101,115,116,97,116,115,0];
        // CoreOwner::empty terminates the owned appCmdLine buffer before publication.
        i32::from(unsafe { hp2_core::search::parse_param(self.0.command_line().as_ptr().cast(), PARAM.as_ptr()) })
    }
    fn delete_connection(&mut self, connection: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.0.delete_object(connection.cast()) }
    }
    fn low_level_destroy(&mut self, driver: *mut UNetDriver) -> RuntimeResult<()> {
        let demo = self.0.native_class("Engine.DemoRecDriver")?;
        if !unsafe { object_model::is_a(driver.cast(), demo.cast()) } {
            return Err(RuntimeError::new("NetDriver has no native LowLevelDestroy implementation"));
        }
        unsafe { crate::demo_owner::low_level_destroy(self.0, driver) }
    }
    fn delete_package_map(&mut self, map: *mut c_void) -> RuntimeResult<()> {
        if map.is_null() { return Ok(()); }
        unsafe { self.0.delete_object(map.cast()) }
    }
    fn destroy_object(&mut self, driver: *mut UNetDriver) -> RuntimeResult<()> {
        unsafe { (OBJECT.destroy)(self.0, driver.cast()) }
    }
}

/// Dispatches `UNetDriver::InitListen` through the actual native class owner.
///
/// The base implementation only stores its notify object. `UDemoRecDriver`
/// overrides it with the recording-driver transition owned by `demo_owner`.
/// Other concrete drivers must provide their own native implementation; they
/// are never silently treated as the base class.
///
/// # Safety
/// `driver` is a live canonical NetDriver (or subclass), and `notify`, `url`,
/// and `error` are the live arguments supplied to its virtual InitListen.
pub unsafe fn init_listen(
    access: CoreAccess<'_>,
    driver: *mut UNetDriver,
    notify: *mut c_void,
    url: *mut FURL,
    error: *mut TArrayRaw,
) -> RuntimeResult<bool> {
    if driver.is_null() || url.is_null() || error.is_null() {
        return Err(RuntimeError::new("Invalid NetDriver InitListen arguments"));
    }
    let driver_class = access.native_class("Engine.NetDriver")?;
    let demo_class = access.native_class("Engine.DemoRecDriver")?;
    let object = driver.cast::<UObject>();
    let class = unsafe { (*object).class };
    if class.is_null() || !unsafe { object_model::is_a(object.cast(), driver_class.cast()) } {
        return Err(RuntimeError::new("InitListen receiver is not an Engine.NetDriver"));
    }
    if unsafe { object_model::is_a(object.cast(), demo_class.cast()) } {
        return unsafe { crate::demo_owner::init_listen(access, driver.cast(), notify, url, error) };
    }
    Ok(unsafe { crate::net_driver::init_notify(driver, notify) })
}

/// Ticks the server and client connections of a canonical network driver.
///
/// # Safety
/// The driver and each connection in its arrays must be live canonical records.
pub unsafe fn tick_flush(
    access: CoreAccess<'_>,
    driver: *mut UNetDriver,
) -> RuntimeResult<()> {
    if driver.is_null() {
        return Err(RuntimeError::new("Null canonical UNetDriver"));
    }
    unsafe {
        crate::net_driver::tick_flush_with(driver, |connection| unsafe {
            crate::connection_owner::tick(access, connection.cast())
        })
    }
}

/// Advances canonical network time and removes closed client connections.
///
/// # Safety
/// The driver and all connections in its arrays are live canonical records.
pub unsafe fn tick_dispatch(
    access: CoreAccess<'_>,
    driver: *mut UNetDriver,
    delta: f32,
) -> RuntimeResult<()> {
    if driver.is_null() {
        return Err(RuntimeError::new("Null canonical UNetDriver"));
    }
    unsafe {
        crate::net_driver::tick_dispatch_with(driver, delta, &mut |connection| {
            access.delete_object(connection.cast())
        })
    }
}

unsafe fn construct_driver(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (SUBSYSTEM.construct)(access, object)?;
        let driver = object.cast::<UNetDriver>();
        ptr::addr_of_mut!((*driver).client_connections).write_unaligned(mem::zeroed());
        (*driver).time = [0; 8];
        crate::net_driver::construct_with_ops(driver, &mut DriverOps(access))
    }
}
unsafe fn destroy_driver(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let driver = object.cast::<UNetDriver>();
    unsafe { crate::net_driver::destroy_with_ops(driver, &mut DriverOps(access)) }
}

pub(crate) unsafe fn finalize_driver_members(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UNetDriver")); }
    let driver = object.cast::<UNetDriver>();
    unsafe { array_storage::release(slots(ptr::addr_of_mut!((*driver).client_connections)),
        array_storage::system_allocator()) }.map_err(array_error)
}

unsafe fn serialize_connections(archive: &ObjectArchive<'_>, array: *mut TArrayRaw) -> RuntimeResult<()> {
    let mut count = unsafe { (*array).num };
    if !matches!(archive.mode(), ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports) {
        let width = mem::size_of::<*mut UObject>();
        archive.count_bytes(count as usize * width, unsafe { (*array).max } as usize * width)?;
        archive.serialize_compact_index(&mut count)?;
        if archive.is_loading() {
            if count < 0 { return Err(RuntimeError::new("Invalid network connection array count")); }
            let allocator = array_storage::system_allocator();
            unsafe {
                array_storage::empty(slots(array), count, width as i32, ptr::null(), allocator).map_err(array_error)?;
                array_storage::add_zeroed(slots(array), count, width as i32, ptr::null(), allocator).map_err(array_error)?;
            }
        }
    }
    for index in 0..count as usize {
        let slot = unsafe { (*array).data.cast::<*mut UObject>().add(index) };
        unsafe { archive.serialize_object_slot(slot)?; }
    }
    Ok(())
}
unsafe fn serialize_driver(access: CoreAccess<'_>, object: *mut UObject, archive: &ObjectArchive<'_>) -> RuntimeResult<()> {
    unsafe { (OBJECT.serialize)(access, object, archive)?; }
    let driver = object.cast::<UNetDriver>();
    unsafe {
        serialize_connections(archive, ptr::addr_of_mut!((*driver).client_connections))?;
        for slot in [ptr::addr_of_mut!((*driver).server_connection), ptr::addr_of_mut!((*driver).master_map),
            ptr::addr_of_mut!((*driver).role_property), ptr::addr_of_mut!((*driver).remote_role_property)] {
            archive.serialize_object_slot(slot.cast())?;
        }
    }
    Ok(())
}
unsafe fn destroy_pending(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { crate::level_owner::destroy_level_base_body(access, object) }
}

unsafe fn finalize_pending(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UPendingLevel")); }
    let pending = object.cast::<UPendingLevel>();
    let mut result = Ok(());
    for string in unsafe { [ptr::addr_of_mut!((*pending).fail_url), ptr::addr_of_mut!((*pending).fail_code), ptr::addr_of_mut!((*pending).error)] } {
        let member = unsafe { access.destroy_owned_fstring(string) };
        result = result.and(member);
    }
    result.and(unsafe { (crate::level_owner::LEVEL_BASE_OPERATIONS.finalize_members)(access, object) })
}

/// Looks up the actual actor channel using UObject's index hash and the shared map chain.
///
/// # Safety
/// Connection, Actor and the connection's canonical actor-channel map remain live.
pub unsafe fn actor_channel(connection: *mut UNetConnection, actor: *mut AActor) -> RuntimeResult<*mut UChannel> {
    let map = unsafe { ptr::addr_of!((*connection).actor_channels) };
    let bucket = unsafe { (*actor.cast::<UObject>()).index & ((*map).hash_count - 1) } as usize;
    let first = unsafe { (*map).hash.cast::<i32>().add(bucket).read_unaligned() };
    let index = hp2_core::map_storage::find_index_with(first,
        |index| Ok::<bool, RuntimeError>(unsafe {
            ptr::addr_of!((*(*map).pairs.data.cast::<FActorChannelMapPair>().add(index as usize)).key)
                .read_unaligned() == actor.cast()
        }),
        |index| Ok::<i32, RuntimeError>(unsafe {
            ptr::addr_of!((*(*map).pairs.data.cast::<FActorChannelMapPair>().add(index as usize)).hash_next).read_unaligned()
        }))?;
    if index == -1 { return Ok(ptr::null_mut()); }
    Ok(unsafe { ptr::addr_of!((*(*map).pairs.data.cast::<FActorChannelMapPair>().add(index as usize)).value)
        .read_unaligned().cast() })
}
impl crate::net_driver::DriverActorOps for DriverOps<'_> {
    type Error = RuntimeError;
    fn remove_sent_temporary(&mut self, connection: *mut c_void, actor: *mut c_void) -> RuntimeResult<()> {
        let array = unsafe { ptr::addr_of_mut!((*connection.cast::<UNetConnection>()).sent_temporaries) };
        let mut index = unsafe { (*array).num } - 1;
        while index >= 0 {
            if unsafe { (*array).data.cast::<*mut c_void>().add(index as usize).read_unaligned() } == actor {
                unsafe { array_storage::remove(slots(array), index, 1, mem::size_of::<*mut c_void>() as i32,
                    ptr::null(), array_storage::system_allocator()) }.map_err(array_error)?;
            }
            index -= 1;
        }
        Ok(())
    }
    fn actor_channel(&mut self, connection: *mut c_void, actor: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { actor_channel(connection.cast(), actor.cast())? }.cast())
    }
    fn close_locally_opened_channel(&mut self, channel: *mut c_void) -> RuntimeResult<()> {
        if unsafe { (*channel.cast::<UChannel>()).opened_locally } == 0 {
            return Err(RuntimeError::new("Actor channel was not opened locally"));
        }
        unsafe { crate::channel_owner::close(self.0, channel.cast()) }
    }
}

/// Applies the shared native network-driver actor-destruction notification policy.
///
/// # Safety
/// Driver, Actor and every reached canonical connection/channel remain live through callbacks.
pub unsafe fn notify_actor_destroyed(access: CoreAccess<'_>, driver: *mut UNetDriver, actor: *mut AActor) -> RuntimeResult<()> {
    unsafe { crate::net_driver::notify_actor_destroyed_with_ops(driver, actor.cast(), &mut DriverOps(access)) }
}

struct CancelOps<'a>(CoreAccess<'a>);
impl crate::game_engine::CancelPendingOps for CancelOps<'_> {
    type Error = RuntimeError;
    fn pending(&mut self, engine: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*engine.cast::<UGameEngine>()).g_pending_level })
    }
    fn driver(&mut self, pending: *mut c_void) -> RuntimeResult<*mut c_void> {
        let demo = self.0.native_class("Engine.DemoPlayPendingLevel")?;
        let net = self.0.native_class("Engine.NetPendingLevel")?;
        unsafe {
            if object_model::is_a(pending, demo.cast()) { return Ok((*pending.cast::<UPendingLevel>()).demo_rec_driver); }
            if object_model::is_a(pending, net.cast()) { return Ok((*pending.cast::<UPendingLevel>()).net_driver); }
        }
        Err(RuntimeError::new("Pending level has no native GetDriver implementation"))
    }
    fn server_connection(&mut self, driver: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*driver.cast::<UNetDriver>()).server_connection })
    }
    fn channel_zero(&mut self, connection: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*connection.cast::<UNetConnection>()).channels[0] })
    }
    fn close_channel(&mut self, channel: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::close(self.0, channel.cast()) }
    }
    fn flush_connection(&mut self, connection: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::flush_net(self.0, connection.cast()) }
    }
    fn delete_pending(&mut self, pending: *mut c_void) -> RuntimeResult<()> {
        if pending.is_null() { return Ok(()); }
        unsafe { self.0.delete_object(pending.cast()) }
    }
    fn set_pending(&mut self, engine: *mut c_void, pending: *mut c_void) -> RuntimeResult<()> {
        unsafe { (*engine.cast::<UGameEngine>()).g_pending_level = pending; }
        Ok(())
    }
}

/// Cancels the live pending graph, rereading it after virtual channel Close.
///
/// # Safety
/// GameEngine and every reached pending/driver/connection/channel identity are canonical and live.
pub unsafe fn cancel_game_pending(access: CoreAccess<'_>, game: *mut UGameEngine) -> RuntimeResult<()> {
    crate::game_engine::cancel_pending_with_ops(game.cast(), &mut CancelOps(access))
}

/// Runs the dynamic source driver Exec without a native vptr or output device.
///
/// # Safety
/// Driver, command and the reached connection/channel graph are canonical and live.
pub unsafe fn exec(
    access: CoreAccess<'_>, driver: *mut UNetDriver, command: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if driver.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid NetDriver command arguments"));
    }
    let demo = access.native_class("Engine.DemoRecDriver")?;
    if unsafe { object_model::is_a(driver.cast(), demo.cast()) } {
        return unsafe { crate::demo_owner::exec(access, driver.cast(), command, output) };
    }
    unsafe { crate::net_driver::exec_sockets_with(&mut CoreSocketOps { access, output }, driver, command) }
}

struct CoreSocketOps<'access, 'borrow, 'output> {
    access: CoreAccess<'access>,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
}

impl crate::net_driver::SocketExecOps for CoreSocketOps<'_, '_, '_> {
    type Error = RuntimeError;
    unsafe fn matches(&mut self, command: *const u32) -> RuntimeResult<bool> {
        let mut cursor = command.cast::<hp2_core::ansi::Tchar>();
        let word = b"SOCKETS\0".map(i32::from);
        Ok(unsafe { hp2_core::command::parse_command(&mut cursor, word.as_ptr()) != 0 })
    }
    fn log_heading(&mut self) -> RuntimeResult<()> {
        (self.output)(&b"Connections:".map(u32::from))
    }
    unsafe fn log_connection(&mut self, connection: *mut UNetConnection, server: bool) -> RuntimeResult<()> {
        let class = self.access.native_class("Engine.DemoRecConnection")?;
        if !unsafe { object_model::is_a(connection.cast(), class.cast()) } {
            return Err(RuntimeError::new("NetConnection has no canonical LowLevelDescribe implementation"));
        }
        let prefix = if server { &b"   Server "[..] } else { &b"   Client "[..] };
        let description = b"Demo recording driver connection";
        let mut text = Vec::with_capacity(prefix.len() + description.len());
        text.extend(prefix.iter().copied().map(u32::from));
        text.extend(description.iter().copied().map(u32::from));
        (self.output)(&text)
    }
    unsafe fn log_channel(&mut self, channel: *mut UChannel) -> RuntimeResult<()> {
        if channel.is_null() { return Err(RuntimeError::new("Null channel in SOCKETS")); }
        let mut text = Vec::with_capacity(96);
        text.extend(b"      Channel ".iter().copied().map(u32::from));
        crate::net_chan::append_description_number(&mut text, unsafe { (*channel).ch_index });
        text.extend(b": ".iter().copied().map(u32::from));
        let prefix = text.len();
        let mut ops = CoreChannelDescriptionOps { access: self.access, text, prefix };
        let actor = self.access.native_class("Engine.ActorChannel")?;
        let file = self.access.native_class("Engine.FileChannel")?;
        let control = self.access.native_class("Engine.ControlChannel")?;
        unsafe {
            if object_model::is_a(channel.cast(), actor.cast()) {
                crate::net_chan::describe_actor_with(&mut ops, channel.cast())?;
            } else if object_model::is_a(channel.cast(), file.cast()) {
                crate::net_chan::describe_file_with(&mut ops, channel.cast())?;
            } else if object_model::is_a(channel.cast(), control.cast()) {
                crate::net_chan::describe_control_with(&mut ops, channel)?;
            } else {
                crate::net_chan::describe_base_with(&mut ops, channel)?;
            }
        }
        (self.output)(&ops.text)
    }
}

struct CoreChannelDescriptionOps<'a> {
    access: CoreAccess<'a>,
    text: Vec<u32>,
    prefix: usize,
}

impl crate::net_chan::ChannelDescriptionOps for CoreChannelDescriptionOps<'_> {
    type Error = RuntimeError;
    fn set_text(&mut self, text: &[u32]) -> RuntimeResult<()> {
        self.text.truncate(self.prefix);
        self.append_text(text)
    }
    fn append_text(&mut self, text: &[u32]) -> RuntimeResult<()> {
        let end = text.iter().position(|unit| *unit == 0).unwrap_or(text.len());
        self.text.extend_from_slice(&text[..end]);
        Ok(())
    }
    unsafe fn set_actor_prefix(&mut self, actor: *mut AActor) -> RuntimeResult<()> {
        let (role, remote_role) = unsafe { ((*actor).role, (*actor).remote_role) };
        let name = unsafe { self.access.object_full_name(actor.cast())? };
        self.text.truncate(self.prefix);
        self.text.extend(b"Actor=".iter().copied().map(u32::from));
        self.text.extend_from_slice(&name);
        self.text.extend(b" (Role=".iter().copied().map(u32::from));
        crate::net_chan::append_description_number(&mut self.text, i32::from(role));
        self.text.extend(b" RemoteRole=".iter().copied().map(u32::from));
        crate::net_chan::append_description_number(&mut self.text, i32::from(remote_role));
        self.text.extend(b") ".iter().copied().map(u32::from));
        Ok(())
    }
    unsafe fn package_file_size(&mut self, connection: *mut UNetConnection, index: i32) -> RuntimeResult<i32> {
        let info = unsafe { crate::file_channel_owner::package_info(connection, index)? };
        Ok(unsafe { (*info).file_size })
    }
    unsafe fn set_file_prefix(&mut self, filename: *const u32, receiving: bool, transferred: i32, size: i32) -> RuntimeResult<()> {
        self.text.truncate(self.prefix);
        unsafe { crate::net_chan::append_file_description(&mut self.text, filename, receiving, transferred, size); }
        Ok(())
    }
}

/// `UDemoRecDriver` returns an empty network number; `UNetDriver` is pure virtual.
pub(crate) unsafe fn low_level_network_number(access: CoreAccess<'_>, driver: *mut UNetDriver) -> RuntimeResult<&'static [u32]> {
    let demo = access.native_class("Engine.DemoRecDriver")?;
    if unsafe { object_model::is_a(driver.cast(), demo.cast()) } {
        Ok(&[])
    } else {
        Err(RuntimeError::new("Pure virtual UNetDriver::LowLevelGetNetworkNumber"))
    }
}
