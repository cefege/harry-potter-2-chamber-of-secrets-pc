//! Canonical Rust ownership for `UDemoRecConnection`, `UDemoRecDriver`, and demo files.

use hp2_abi::engine::generated::{FURL, TArrayRaw, UClass, UDemoRecConnection, UDemoRecDriver, ULevel, UNetConnection, UNetDriver, UObject};
use hp2_core::{array_storage, core_access::{CoreAccess, RuntimeError, RuntimeResult}, core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind}, object_owner::ClassOps};
use std::alloc::Layout;
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::ptr;
const OPT_3RDPERSON: [u32; 10] = [51, 114, 100, 112, 101, 114, 115, 111, 110, 0];
const OPT_TIMEBASED: [u32; 10] = [116, 105, 109, 101, 98, 97, 115, 101, 100, 0];
const OPT_NOFRAMECAP: [u32; 11] = [110, 111, 102, 114, 97, 109, 101, 99, 97, 112, 0];
const OPT_LOOP: [u32; 5] = [108, 111, 111, 112, 0];

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_TRANSIENT: u32 = 0x0000_0008;
const CLASS_CONFIG: u32 = 0x0000_0004;
const ARCHIVE_TAG: usize = 5;
const ARCHIVE_TAG_MASK: usize = 7;

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "DemoRecConnection", package: "Engine", super_name: Some("NetConnection"),
        within: "Object", flags: CLASS_TRANSIENT | CLASS_CONFIG, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UDemoRecConnection>()), operations: Some(CONNECTION_OPERATIONS),
        properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "DemoRecDriver", package: "Engine", super_name: Some("NetDriver"),
        within: "Object", flags: CLASS_TRANSIENT | CLASS_CONFIG, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UDemoRecDriver>()), operations: Some(DRIVER_OPERATIONS),
        properties: &[StartupProperty { name: "DemoSpectatorClass", kind: StartupPropertyKind::String, category: "Client", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UDemoRecDriver, demo_spectator_class), array_dim: 1 }], defaults: &[],
    },
];

const CONNECTION_OPERATIONS: ClassOps = {
    let mut ops = crate::connection_owner::CONNECTION_OPERATIONS;
    ops.construct = construct_demo_connection;
    ops.destroy = destroy_demo_connection;
    ops
};

const DRIVER_OPERATIONS: ClassOps = {
    let mut ops = crate::network_owner::DRIVER_OPERATIONS;
    ops.construct = construct_demo_driver;
    ops.destroy = destroy_demo_driver;
    ops.finalize_members = finalize_demo_driver;
    ops
};

struct DemoArchive { file: DemoFile, read_error: Option<std::io::Error> }
enum DemoFile { Reader(BufReader<File>), Writer(BufWriter<File>) }
impl DemoFile {
    fn flush(&mut self) -> RuntimeResult<()> {
        if let Self::Writer(file) = self { file.flush().map_err(|e| io_error("Demo file flush failed", e))?; }
        Ok(())
    }
}

fn io_error(action: &str, error: std::io::Error) -> RuntimeError {
    RuntimeError::new(action).with_context(&error.to_string())
}

unsafe fn archive_ptr(driver: *mut UDemoRecDriver) -> *mut c_void {
    unsafe { ptr::addr_of!((*driver).file_ar).read_unaligned() }
}
unsafe fn set_archive_ptr(driver: *mut UDemoRecDriver, value: *mut c_void) {
    unsafe { ptr::addr_of_mut!((*driver).file_ar).write_unaligned(value); }
}
unsafe fn archive_from_handle<'a>(raw: *mut c_void) -> RuntimeResult<&'a mut DemoArchive> {
    if raw.is_null() || raw.addr() & ARCHIVE_TAG_MASK != ARCHIVE_TAG {
        return Err(RuntimeError::new("Invalid demo archive handle"));
    }
    // SAFETY: the tagged pointer is installed only by open_archive and remains owned by the driver.
    Ok(unsafe { &mut *raw.map_addr(|address| address & !ARCHIVE_TAG_MASK).cast::<DemoArchive>() })
}

unsafe fn fstring_units(value: *const TArrayRaw) -> RuntimeResult<Vec<u32>> {
    let array = unsafe { value.read_unaligned() };
    if array.num < 0 || array.max < array.num {
        return Err(RuntimeError::new("Invalid demo filename FString"));
    }
    if array.num == 0 { return Ok(vec![0]); }
    if array.data.is_null() { return Err(RuntimeError::new("Demo filename FString has null storage")); }
    Ok(unsafe { std::slice::from_raw_parts(array.data.cast::<u32>(), array.num as usize) }.to_vec())
}

fn demo_filename(driver: *mut UDemoRecDriver) -> RuntimeResult<String> {
    let units = unsafe { fstring_units(ptr::addr_of!((*driver).demo_filename))? };
    units.iter().copied().take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Demo filename contains an invalid character")))
        .collect()
}

unsafe fn url_map_path(url: *mut FURL) -> RuntimeResult<(Vec<u32>, std::path::PathBuf)> {
    if url.is_null() { return Err(RuntimeError::new("Null demo URL")); }
    let map = unsafe { ptr::addr_of!((*url).map).read_unaligned() };
    let units = unsafe { fstring_units(&map) }?;
    let text = units.iter().copied().take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Demo filename contains an invalid character")))
        .collect::<RuntimeResult<String>>()?;
    Ok((units, text.into()))
}

unsafe fn set_demo_filename_from_url(access: CoreAccess<'_>, driver: *mut UDemoRecDriver, url: *mut FURL) -> RuntimeResult<std::path::PathBuf> {
    let (units, path) = unsafe { url_map_path(url) }?;
    unsafe { access.assign_fstring(ptr::addr_of_mut!((*driver).demo_filename), &units)?; }
    Ok(path)
}

unsafe fn copy_url(access: CoreAccess<'_>, source: *mut FURL, destination: *mut FURL) -> RuntimeResult<()> {
    unsafe { crate::url_owner::UrlOwner::assign_copy(access, destination, source) }
}

unsafe fn open_archive(driver: *mut UDemoRecDriver, path: &Path, record: bool) -> RuntimeResult<()> {
    let opened = if record {
        OpenOptions::new().write(true).create(true).truncate(true).open(path)
    } else {
        OpenOptions::new().read(true).open(path)
    };
    let filename = path.display().to_string();
    let file = opened.map_err(|error| io_error(if record { "Could not open demo file for writing" } else { "Could not open demo file for reading" }, error).with_context(&filename))?;
    let state = if record { DemoFile::Writer(BufWriter::new(file)) } else { DemoFile::Reader(BufReader::new(file)) };
    let boxed = Box::into_raw(Box::new(DemoArchive { file: state, read_error: None }));
    debug_assert_eq!(boxed.addr() & ARCHIVE_TAG_MASK, 0);
    unsafe { set_archive_ptr(driver, boxed.map_addr(|address| address | ARCHIVE_TAG).cast()); }
    Ok(())
}

struct CoreDemoOps<'a>(CoreAccess<'a>);
impl crate::net_demo::DemoInitOps for CoreDemoOps<'_> {
    type Error = RuntimeError;
    unsafe fn init_base(&self, this: *mut c_void, url: *mut c_void) -> RuntimeResult<()> {
        let driver = this.cast::<UDemoRecDriver>();
        let _path = unsafe { set_demo_filename_from_url(self.0, driver, url.cast())? };
        unsafe {
            ptr::addr_of_mut!((*driver).base.time).write_unaligned([0; 8]);
            (*driver).frame_num = 0;
            (*driver).demo_ended = 0;
        }
        Ok(())
    }
    unsafe fn new_connection(&self, this: *mut c_void, url: *mut c_void) -> RuntimeResult<*mut c_void> {
        let driver = this.cast::<UDemoRecDriver>();
        let class = self.0.native_class("Engine.DemoRecConnection")?;
        let connection = unsafe {
            self.0.construct_object_with_constructor(
                class.cast(), self.0.transient_package(), 0, 0, ptr::null_mut(),
                |access, object| {
                    crate::connection_owner::construct_parameterized(access, object.cast(), driver.cast(), url.cast())?;
                    (*object.cast::<UDemoRecConnection>()).base.max_packet = 512;
                    (*object.cast::<UDemoRecConnection>()).base.internal_ack = 1;
                    Ok(())
                },
            )?
        };
        Ok(connection.cast())
    }
}
impl crate::net_demo::DemoConnectOps for CoreDemoOps<'_> {
    unsafe fn super_init_connect(&self, this: *mut c_void, notify: *mut c_void, _url: *mut c_void, _err: *mut c_void) -> RuntimeResult<bool> {
        Ok(unsafe { crate::net_driver::init_notify(this.cast::<UNetDriver>(), notify) })
    }
    unsafe fn open_reader(&self, this: *mut c_void) -> RuntimeResult<bool> {
        let driver = this.cast::<UDemoRecDriver>();
        let units = unsafe { fstring_units(ptr::addr_of!((*driver).demo_filename))? };
        let filename = units.iter().copied().take_while(|unit| *unit != 0)
            .map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Demo filename contains an invalid character")))
            .collect::<RuntimeResult<String>>()?;
        unsafe { set_archive_ptr(driver, ptr::null_mut()); }
        Ok(unsafe { open_archive(driver, Path::new(&filename), false) }.is_ok())
    }
    unsafe fn set_open_error(&self, this: *mut c_void, err: *mut c_void) -> RuntimeResult<()> {
        let driver = this.cast::<UDemoRecDriver>();
        let units = unsafe { fstring_units(ptr::addr_of!((*driver).demo_filename))? };
        let filename = units.iter().copied().take_while(|unit| *unit != 0)
            .map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Demo filename contains an invalid character")))
            .collect::<RuntimeResult<String>>()?;
        let message = format!("Couldn't open demo file {filename} for reading");
        let wide: Vec<u32> = message.chars().map(u32::from).chain(std::iter::once(0)).collect();
        unsafe { self.0.assign_fstring(err.cast(), &wide) }
    }
    unsafe fn copy_loop_url(&self, this: *mut c_void, url: *mut c_void) -> RuntimeResult<()> {
        let driver = this.cast::<UDemoRecDriver>();
        unsafe { copy_url(self.0, url.cast(), ptr::addr_of_mut!((*driver).loop_url)) }
    }
    unsafe fn has_option(&self, url: *mut c_void, option: *const c_void) -> RuntimeResult<i32> {
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.0)?;
        Ok(i32::from(unsafe { crate::url_owner::UrlOwner::has_option(self.0, &defaults, url.cast(), option.cast())? }))
    }
}

/// Performs the native `InitConnect` transition with a Core-owned parameterized
/// connection and a real demo file reader.
///
/// # Safety
/// Driver, notification subobject, URL and error FString are live canonical objects.
pub unsafe fn init_connect(
    access: CoreAccess<'_>,
    driver: *mut UDemoRecDriver,
    notify: *mut c_void,
    url: *mut FURL,
    error: *mut TArrayRaw,
) -> RuntimeResult<bool> {
    if driver.is_null() || url.is_null() || error.is_null() { return Err(RuntimeError::new("Invalid demo InitConnect arguments")); }
    unsafe {
        crate::net_demo::driver_init_connect_with(&CoreDemoOps(access), driver.cast(), notify, url.cast(), error.cast())
            .map(|result| result != 0)
    }
}

impl crate::net_demo::DemoListenOps for CoreDemoOps<'_> {
    unsafe fn super_init_listen(&self, this: *mut c_void, notify: *mut c_void, _url: *mut c_void, _err: *mut c_void) -> RuntimeResult<bool> {
        Ok(unsafe { crate::net_driver::init_notify(this.cast::<UNetDriver>(), notify) })
    }
    unsafe fn init_out(&self, connection: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::init_out(self.0, connection.cast()) }
    }
    unsafe fn open_writer(&self, this: *mut c_void) -> RuntimeResult<bool> {
        let driver = this.cast::<UDemoRecDriver>();
        let filename = unsafe { demo_filename(driver)? };
        unsafe { set_archive_ptr(driver, ptr::null_mut()); }
        Ok(unsafe { open_archive(driver, Path::new(&filename), true) }.is_ok())
    }
    unsafe fn add_client(&self, this: *mut c_void, connection: *mut c_void) -> RuntimeResult<()> {
        let driver = this.cast::<UNetDriver>();
        let clients = unsafe { ptr::addr_of_mut!((*driver).client_connections) };
        let slots = unsafe { hp2_core::array_storage::Hp2ArraySlots {
            data: ptr::addr_of_mut!((*clients).data),
            num: ptr::addr_of_mut!((*clients).num),
            max: ptr::addr_of_mut!((*clients).max),
        } };
        let index = unsafe {
            array_storage::add(slots, 1, std::mem::size_of::<*mut c_void>() as i32, ptr::null(), array_storage::system_allocator())
        }.map_err(|code| RuntimeError::new("Failed to add demo client connection").with_context(&code.to_string()))?;
        let data = unsafe { ptr::addr_of!((*clients).data).read_unaligned() };
        if data.is_null() { return Err(RuntimeError::new("Demo client connection array has null storage")); }
        unsafe { data.cast::<*mut c_void>().add(index as usize).write_unaligned(connection); }
        Ok(())
    }
    unsafe fn set_write_error(&self, this: *mut c_void, err: *mut c_void) -> RuntimeResult<()> {
        let filename = unsafe { demo_filename(this.cast())? };
        let message = format!("Couldn't open demo file {filename} for writing");
        let wide: Vec<u32> = message.chars().map(u32::from).chain(std::iter::once(0)).collect();
        unsafe { self.0.assign_fstring(err.cast(), &wide) }
    }
    unsafe fn driver_get_level(&self, this: *mut c_void) -> RuntimeResult<*mut c_void> {
        unsafe { notify_get_level(self.0, this.cast()) }.map(|level| level.cast())
    }
    unsafe fn level_engine(&self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        if level.is_null() { return Err(RuntimeError::new("Demo listen requires a loaded level")); }
        Ok(unsafe { (*level.cast::<ULevel>()).base.engine })
    }
    unsafe fn level_netmode(&self, level: *mut c_void) -> RuntimeResult<i32> {
        let info = unsafe { crate::level_owner::actor_at(level.cast(), 0) };
        if info.is_null() { return Err(RuntimeError::new("Demo listen level has no LevelInfo actor")); }
        Ok(unsafe { (*info.cast::<hp2_abi::engine::generated::ALevelInfo>()).net_mode as i32 })
    }
    unsafe fn level_server_package_map(&self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        let level = level.cast::<ULevel>();
        if level.is_null() { return Err(RuntimeError::new("Demo listen requires a loaded level")); }
        let driver = unsafe { (*level).base.net_driver.cast::<UNetDriver>() };
        if driver.is_null() { return Err(RuntimeError::new("Demo listen level has no network driver")); }
        Ok(unsafe { (*driver).master_map })
    }
    unsafe fn driver_copy_linkers(&self, this: *mut c_void, source: *mut c_void) -> RuntimeResult<()> {
        let destination = unsafe { (*this.cast::<UNetDriver>()).master_map.cast::<hp2_abi::engine::generated::UPackageMap>() };
        if destination.is_null() || source.is_null() {
            return Err(RuntimeError::new("Demo listen package maps are not initialized"));
        }
        unsafe { crate::package_map_level_owner::copy_linkers(self.0, destination, source.cast()) }
    }
    unsafe fn load_spectator_class(&self) -> RuntimeResult<*mut c_void> {
        let actor_class = self.0.native_class("Engine.Actor")?;
        unsafe {
            self.0.static_load_class(
                actor_class.cast(),
                ptr::null_mut(),
                &crate::net_demo::DEMO_SPECTATOR_CLASS,
                1,
            ).map(|class| class.cast())
        }
    }
    unsafe fn spawn_spectator(&self, level: *mut c_void, class: *mut c_void) -> RuntimeResult<()> {
        if level.is_null() || class.is_null() {
            return Err(RuntimeError::new("Demo spectator spawn received a null Level or Class"));
        }
        unsafe {
            crate::level_spawn_actor_owner::spawn_actor(
                self.0, level.cast(), class.cast(), hp2_abi::engine::generated::FName { index: 0 },
                ptr::null_mut(), ptr::null_mut(),
                hp2_abi::FVector { x: 0.0, y: 0.0, z: 0.0 },
                hp2_abi::engine::generated::FRotator { pitch: 0, yaw: 0, roll: 0 },
                ptr::null_mut(), false, false,
            )?;
        }
        Ok(())
    }
    unsafe fn build_server_master_map(&self, engine: *mut c_void, driver: *mut c_void, level: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::package_map_level_owner::build_server_master_map(
            self.0, engine.cast(), driver.cast(), level.cast(),
        ) }
    }
    unsafe fn create_control_channel(&self, connection: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::channel_owner::create_control_channel(self.0, connection.cast()).map(|_| ()) }
    }
    unsafe fn welcome_player(&self, level: *mut c_void, connection: *mut c_void, optional: *const c_void) -> RuntimeResult<()> {
        unsafe { crate::level_net_owner::welcome_player(self.0, level.cast(), connection.cast(), optional.cast()) }
    }
}

/// Performs the complete native `InitListen` source transition, including the
/// recording connection, archive, level package-map setup, control channel and welcome.
///
/// # Safety
/// Driver, notification subobject, URL and error FString are live canonical objects.
pub unsafe fn init_listen(
    access: CoreAccess<'_>,
    driver: *mut UDemoRecDriver,
    notify: *mut c_void,
    url: *mut FURL,
    error: *mut TArrayRaw,
) -> RuntimeResult<bool> {
    if driver.is_null() || url.is_null() || error.is_null() { return Err(RuntimeError::new("Invalid demo InitListen arguments")); }
    unsafe {
        crate::net_demo::driver_init_listen_with(&CoreDemoOps(access), driver.cast(), notify, url.cast(), error.cast())
            .map(|result| result != 0)
    }
}

unsafe fn close_archive(driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
    let raw = unsafe { archive_ptr(driver) };
    if raw.is_null() { return Ok(()); }
    let archive = unsafe { archive_from_handle(raw)? };
    let flush_result = archive.file.flush();
    unsafe {
        drop(Box::from_raw(raw.map_addr(|address| address & !ARCHIVE_TAG_MASK).cast::<DemoArchive>()));
        set_archive_ptr(driver, ptr::null_mut());
    }
    flush_result
}

unsafe fn construct_demo_connection(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::connection_owner::CONNECTION_OPERATIONS.construct)(access, object) }
}

unsafe fn destroy_demo_connection(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::connection_owner::CONNECTION_OPERATIONS.destroy)(access, object) }
}
unsafe fn construct_demo_driver(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (crate::network_owner::DRIVER_OPERATIONS.construct)(access, object)?;
        let driver = object.cast::<UDemoRecDriver>();
        let defaults = crate::url_owner::UrlOwner::current_defaults(access)?;
        crate::url_owner::UrlOwner::construct_default(access, &defaults, ptr::addr_of_mut!((*driver).loop_url))?;
    }
    Ok(())
}

unsafe fn destroy_demo_driver(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (crate::network_owner::DRIVER_OPERATIONS.destroy)(access, object) }
}

unsafe fn finalize_demo_driver(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() { return Err(RuntimeError::new("Null canonical UDemoRecDriver")); }
    let driver = object.cast::<UDemoRecDriver>();
    let mut first_error = None;
    remember(&mut first_error, unsafe { crate::url_owner::UrlOwner::destroy(access, ptr::addr_of_mut!((*driver).loop_url)) });
    remember(&mut first_error, unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*driver).demo_filename)) });
    remember(&mut first_error, unsafe { crate::network_owner::finalize_driver_members(access, object) });
    match first_error { Some(error) => Err(error), None => Ok(()) }
}

fn remember(first: &mut Option<RuntimeError>, result: RuntimeResult<()>) {
    if first.is_none() {
        if let Err(error) = result { *first = Some(error); }
    }
}

/// Resolves the driver's notification object by canonical Level/Pending class identity.
///
/// # Safety
/// `driver` is live and its notification pointer names a canonical level or pending-level object.
pub unsafe fn notify_get_level(access: CoreAccess<'_>, driver: *mut UNetDriver) -> RuntimeResult<*mut ULevel> {
    if driver.is_null() { return Err(RuntimeError::new("Null demo network driver")); }
    let notify_base = unsafe { (*driver).notify };
    if notify_base.is_null() { return Err(RuntimeError::new("Demo network driver has no notify target")); }
    // FNetworkNotify is ULevelBase's secondary base, immediately after UObject in the source ABI.
    let notify = notify_base.cast::<u8>().wrapping_sub(std::mem::size_of::<UObject>()).cast::<UObject>();
    let mut class = unsafe { (*notify).class.cast::<UClass>() };
    let level_class = access.native_class("Engine.Level")?;
    let pending_class = access.native_class("Engine.PendingLevel")?;
    while !class.is_null() {
        if class == level_class.cast() { return Ok(notify.cast()); }
        if class == pending_class.cast() { return Ok(ptr::null_mut()); }
        class = unsafe { (*class).super_field.cast() };
    }
    Err(RuntimeError::new("Demo driver notify target is not a canonical level or pending level"))
}

/// Closes and flushes the driver's owned demo archive.
///
/// # Safety
/// `driver` is a live canonical `UDemoRecDriver`.
pub unsafe fn close_file(driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
    if driver.is_null() { return Err(RuntimeError::new("Null demo driver")); }
    unsafe { close_archive(driver) }
}

struct CoreDemoDestroyOps<'a>(CoreAccess<'a>);

/// Logs and closes the Rust-owned demo file archive.
///
/// # Safety
/// `driver` points to a live `UNetDriver` whose dynamic canonical type is `UDemoRecDriver`.
pub unsafe fn low_level_destroy(access: CoreAccess<'_>, driver: *mut UNetDriver) -> RuntimeResult<()> {
    if driver.is_null() { return Err(RuntimeError::new("Null demo driver")); }
    let ops = CoreDemoDestroyOps(access);
    unsafe { crate::net_demo::driver_low_level_destroy_with(&ops, driver.cast()) }
}

impl crate::net_demo::DemoLowLevelDestroyOps for CoreDemoDestroyOps<'_> {
    type Error = RuntimeError;
    unsafe fn log_close(&self) -> RuntimeResult<()> {
        let category = self.0.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
        self.0.emit_log(category, &[
            67, 108, 111, 115, 105, 110, 103, 32, 100, 111, 119, 110, 32,
            100, 101, 109, 111, 32, 100, 114, 105, 118, 101, 114, 46,
        ])
    }
    unsafe fn close_archive(&self, driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
        unsafe { close_archive(driver) }
    }
}


/// Writes one native demo packet record: frame, FTime, byte count, then packet bytes.
///
/// Playback drivers discard outgoing bytes, matching `UDemoRecConnection::LowLevelSend`.
///
/// # Safety
/// `connection` is a live canonical `UNetConnection`; `data` references `count` readable bytes.
pub unsafe fn low_level_send(_access: CoreAccess<'_>, connection: *mut UNetConnection, data: *const u8, count: i32) -> RuntimeResult<()> {
    if connection.is_null() || count < 0 || (count != 0 && data.is_null()) {
        return Err(RuntimeError::new("Invalid demo packet buffer"));
    }
    unsafe {
        crate::net_demo::conn_low_level_send_with(&FilePacketOps, connection, data, count)
    }
}

struct FilePacketOps;
impl crate::net_demo::DemoPacketArchiveOps for FilePacketOps {
    type Error = RuntimeError;
    unsafe fn write_i32(&self, raw: *mut c_void, value: i32) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw)? };
        let DemoFile::Writer(file) = &mut archive.file else { return Err(RuntimeError::new("Demo archive is not writable")); };
        file.write_all(&value.to_le_bytes()).map_err(|error| io_error("Demo packet write failed", error))
    }
    unsafe fn write_time(&self, driver: *mut UDemoRecDriver, raw: *mut c_void) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw)? };
        let DemoFile::Writer(file) = &mut archive.file else { return Err(RuntimeError::new("Demo archive is not writable")); };
        let time = unsafe { ptr::addr_of!((*driver).base.time).read_unaligned() };
        file.write_all(&time).map_err(|error| io_error("Demo packet time write failed", error))
    }
    unsafe fn write_bytes(&self, raw: *mut c_void, data: *const u8, count: i32) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw)? };
        let DemoFile::Writer(file) = &mut archive.file else { return Err(RuntimeError::new("Demo archive is not writable")); };
        if count != 0 {
            // SAFETY: caller guarantees the packet buffer is readable for count bytes.
            let bytes = unsafe { std::slice::from_raw_parts(data, count as usize) };
            file.write_all(bytes).map_err(|error| io_error("Demo packet write failed", error))?;
        }
        Ok(())
    }
}

/// Reads the next native demo packet record for file transport consumers.
///
/// # Safety
/// `driver` is live and playback-initialized; the output buffer has `capacity` writable bytes.
pub unsafe fn read_packet(driver: *mut UDemoRecDriver, output: &mut [u8]) -> RuntimeResult<Option<(i32, i64, usize)>> {
    if driver.is_null() { return Err(RuntimeError::new("Null demo driver")); }
    let raw = unsafe { archive_ptr(driver) };
    let archive = unsafe { archive_from_handle(raw)? };
    let DemoFile::Reader(file) = &mut archive.file else { return Err(RuntimeError::new("Demo archive is not readable")); };
    let mut header = [0u8; 16];
    match file.read(&mut header[..1]) {
        Ok(0) => return Ok(None),
        Ok(_) => file.read_exact(&mut header[1..]).map_err(|error| io_error("Demo packet header read failed", error))?,
        Err(error) => return Err(io_error("Demo packet header read failed", error)),
    }
    let frame = i32::from_le_bytes(header[0..4].try_into().expect("fixed header slice"));
    let time = i64::from_le_bytes(header[4..12].try_into().expect("fixed header slice"));
    let count = i32::from_le_bytes(header[12..16].try_into().expect("fixed header slice"));
    if count < 0 || count as usize > output.len() { return Err(RuntimeError::new("Demo packet exceeds read buffer")); }
    file.read_exact(&mut output[..count as usize]).map_err(|e| io_error("Demo packet read failed", e))?;
    Ok(Some((frame, time, count as usize)))
}


/// Advances actual demo playback through canonical packet ingress.
///
/// # Safety
/// `driver`, its connection and notification target are live canonical records.
pub unsafe fn tick_dispatch(
    access: CoreAccess<'_>, driver: *mut UDemoRecDriver, delta: f32,
) -> RuntimeResult<()> {
    if driver.is_null() { return Err(RuntimeError::new("Null demo driver")); }
    unsafe { crate::net_demo::driver_tick_dispatch_with(&CoreDemoTickOps(access), driver, delta) }
}

struct CoreDemoTickOps<'a>(CoreAccess<'a>);

/// File readers expose short reads as sticky archive errors, matching FArchive.
unsafe fn read_archive_bytes(raw: *mut c_void, output: &mut [u8]) -> RuntimeResult<()> {
    let archive = unsafe { archive_from_handle(raw)? };
    let DemoFile::Reader(reader) = &mut archive.file else {
        return Err(RuntimeError::new("Demo archive is not readable"));
    };
    if let Err(error) = reader.read_exact(output) {
        archive.read_error.get_or_insert(error);
    }
    Ok(())
}

impl crate::net_demo::DemoTickDispatchOps for CoreDemoTickOps<'_> {
    type Error = RuntimeError;
    unsafe fn super_tick_dispatch(&self, driver: *mut UDemoRecDriver, delta: f32) -> RuntimeResult<()> {
        unsafe { crate::network_owner::tick_dispatch(self.0, driver.cast(), delta) }
    }
    unsafe fn archive_at_end(&self, raw: *mut c_void) -> RuntimeResult<bool> {
        let archive = unsafe { archive_from_handle(raw)? };
        let DemoFile::Reader(reader) = &mut archive.file else {
            return Err(RuntimeError::new("Demo archive is not readable"));
        };
        match reader.fill_buf() {
            Ok(buffer) => Ok(buffer.is_empty()),
            Err(error) => {
                archive.read_error.get_or_insert(error);
                Ok(false)
            }
        }
    }
    unsafe fn archive_is_error(&self, raw: *mut c_void) -> RuntimeResult<bool> {
        Ok(unsafe { archive_from_handle(raw)? }.read_error.is_some())
    }
    unsafe fn read_i32(&self, raw: *mut c_void) -> RuntimeResult<i32> {
        let mut bytes = [0; 4];
        unsafe { read_archive_bytes(raw, &mut bytes)?; }
        Ok(i32::from_le_bytes(bytes))
    }
    unsafe fn read_time(&self, raw: *mut c_void) -> RuntimeResult<i64> {
        let mut bytes = [0; 8];
        unsafe { read_archive_bytes(raw, &mut bytes)?; }
        Ok(i64::from_le_bytes(bytes))
    }
    unsafe fn tell(&self, raw: *mut c_void) -> RuntimeResult<i32> {
        let archive = unsafe { archive_from_handle(raw)? };
        let DemoFile::Reader(reader) = &mut archive.file else {
            return Err(RuntimeError::new("Demo archive is not readable"));
        };
        let position = reader.stream_position().map_err(|error| io_error("Demo archive tell failed", error))?;
        i32::try_from(position).map_err(|_| RuntimeError::new("Demo archive position exceeds signed 32-bit range"))
    }
    unsafe fn seek(&self, raw: *mut c_void, position: i32) -> RuntimeResult<()> {
        let position = u64::try_from(position).map_err(|_| RuntimeError::new("Negative demo archive seek"))?;
        let archive = unsafe { archive_from_handle(raw)? };
        let DemoFile::Reader(reader) = &mut archive.file else {
            return Err(RuntimeError::new("Demo archive is not readable"));
        };
        reader.seek(SeekFrom::Start(position)).map_err(|error| io_error("Demo archive seek failed", error))?;
        Ok(())
    }
    fn seconds(&self) -> RuntimeResult<i64> {
        Ok(hp2_core::time::from_double(hp2_core::unix_clock::seconds_slow()))
    }
    unsafe fn read_u32(&self, raw: *mut c_void) -> RuntimeResult<u32> {
        let mut bytes = [0; 4];
        unsafe { read_archive_bytes(raw, &mut bytes)?; }
        Ok(u32::from_le_bytes(bytes))
    }
    unsafe fn read_bytes(&self, raw: *mut c_void, output: &mut [u8]) -> RuntimeResult<()> {
        unsafe { read_archive_bytes(raw, output) }
    }
    unsafe fn mark_archive_error(&self, raw: *mut c_void) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw)? };
        archive.read_error.get_or_insert_with(|| std::io::Error::new(
            std::io::ErrorKind::InvalidData, "Demo packet exceeds protocol read buffer",
        ));
        Ok(())
    }
    fn log_read_failure(&self) -> RuntimeResult<()> {
        let category = self.0.intern_str("DevNet", hp2_core::name_owner::FindName::Add)?;
        let text = b"Failed to read demo file packet".map(u32::from);
        self.0.emit_log(category, &text)
    }
    unsafe fn received_raw_packet(&self, connection: *mut UNetConnection, packet: &mut [u8]) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::received_raw_packet(self.0, connection, packet.as_mut_ptr(), packet.len() as i32) }
    }
    unsafe fn play_loop(&self, driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
        let level = unsafe { notify_get_level(self.0, driver.cast())? };
        if level.is_null() { return Err(RuntimeError::new("Demo driver has no level")); }
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.0)?;
        let mut url = unsafe {
            crate::url_owner::UrlOwner::string(self.0, &defaults, ptr::addr_of!((*driver).loop_url), false)?
        };
        let result = (|| {
            let units = if url.num == 0 { &[][..] } else {
                if url.num < 0 || url.max < url.num || url.data.is_null() {
                    return Err(RuntimeError::new("Invalid demo loop URL string"));
                }
                unsafe { std::slice::from_raw_parts(url.data.cast::<u32>(), url.num as usize) }
            };
            let mut command = Vec::with_capacity(10 + units.len());
            command.extend_from_slice(&[68, 69, 77, 79, 80, 76, 65, 89, 32]);
            command.extend(units.iter().copied().take_while(|unit| *unit != 0));
            command.push(0);
            let category = self.0.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
            let mut output = |text: &[u32]| self.0.emit_log(category, text);
            unsafe { crate::level_exec_owner::exec(self.0, level, command.as_ptr(), &mut output).map(|_| ()) }
        })();
        let cleanup = unsafe { self.0.destroy_owned_fstring(&mut url) };
        result.and(cleanup)
    }
}

/// Executes source Demo commands over a canonical driver.
///
/// # Safety
/// Driver and terminated command are live canonical storage. A recording
/// STOPDEMO deletes the driver; the caller must not dereference it afterwards.
pub unsafe fn exec(
    access: CoreAccess<'_>, driver: *mut UDemoRecDriver, command: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if driver.is_null() || command.is_null() {
        return Err(RuntimeError::new("Invalid demo command arguments"));
    }
    unsafe { crate::net_demo::driver_exec_with(&mut CoreDemoExecOps { access, output }, driver, command) }
}

struct CoreDemoExecOps<'access, 'borrow, 'output> {
    access: CoreAccess<'access>,
    output: &'borrow mut crate::exec_output::ExecOutput<'output>,
}

unsafe fn status_message(driver: *mut UDemoRecDriver, prefix: &[u8]) -> RuntimeResult<Vec<u32>> {
    let filename = unsafe { ptr::addr_of!((*driver).demo_filename).read_unaligned() };
    if filename.num < 0 || filename.max < filename.num || (filename.num > 0 && filename.data.is_null()) {
        return Err(RuntimeError::new("Invalid demo filename FString"));
    }
    let units = if filename.num == 0 { &[][..] } else {
        unsafe { std::slice::from_raw_parts(filename.data.cast::<u32>(), filename.num as usize) }
    };
    let mut message = Vec::with_capacity(prefix.len() + units.len() + 32);
    message.extend(prefix.iter().copied().map(u32::from));
    message.extend(units.iter().copied().take_while(|unit| *unit != 0));
    Ok(message)
}

impl crate::net_demo::DemoExecOps for CoreDemoExecOps<'_, '_, '_> {
    type Error = RuntimeError;
    unsafe fn parse_command(&mut self, cursor: &mut *const u32, word: *const u32) -> RuntimeResult<bool> {
        let mut text = (*cursor).cast::<hp2_core::ansi::Tchar>();
        let result = unsafe { hp2_core::command::parse_command(&mut text, word.cast()) };
        *cursor = text.cast();
        Ok(result != 0)
    }
    unsafe fn log_active(&mut self, driver: *mut UDemoRecDriver, playback: bool) -> RuntimeResult<()> {
        let prefix = if playback { &b"Demo playback currently active: "[..] } else { &b"Demo recording currently active: "[..] };
        let message = unsafe { status_message(driver, prefix)? };
        (self.output)(&message)
    }
    unsafe fn log_stopped(&mut self, driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
        let mut message = unsafe { status_message(driver, b"Demo ")? };
        message.extend(b" stopped (".iter().copied().map(u32::from));
        let frame = unsafe { (*driver).frame_num };
        if frame < 0 { message.push(u32::from(b'-')); }
        let mut number = frame.unsigned_abs();
        let mut digits = [0u32; 10];
        let mut start = digits.len();
        loop {
            start -= 1;
            digits[start] = u32::from(b'0') + number % 10;
            number /= 10;
            if number == 0 { break; }
        }
        message.extend_from_slice(&digits[start..]);
        message.extend(b" frames)".iter().copied().map(u32::from));
        (self.output)(&message)
    }
    unsafe fn clear_recording_driver(&mut self, driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
        let level = unsafe { notify_get_level(self.access, driver.cast())? };
        if level.is_null() { return Err(RuntimeError::new("Demo driver has no level")); }
        unsafe { (*level).base.demo_rec_driver = ptr::null_mut(); }
        Ok(())
    }
    unsafe fn delete_driver(&mut self, driver: *mut UDemoRecDriver) -> RuntimeResult<()> {
        unsafe { self.access.delete_object(driver.cast()) }
    }
}
