//! Direct Core ownership of `ULevel::WelcomePlayer`.

use hp2_abi::engine::generated::{ULevel, UNetConnection, UObject, UNetDriver, UPackageMap, FPackageInfo, UDownload, UChannel, UClass};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, ptr};

/// Backend-specific operations used by the single WelcomePlayer algorithm.
pub(crate) trait WelcomePlayerOps {
    type Error;
    unsafe fn copy_package_map(&mut self, connection: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn send_package_map(&mut self, connection: *mut c_void) -> std::result::Result<(), Self::Error>;
    unsafe fn welcome_values(&mut self, level: *mut c_void) -> std::result::Result<(Vec<u32>, i32), Self::Error>;
    unsafe fn connection_log(&mut self, connection: *mut c_void, message: &[u32]) -> std::result::Result<(), Self::Error>;
    unsafe fn flush_net(&mut self, connection: *mut c_void) -> std::result::Result<(), Self::Error>;
}

/// Runs `ULevel::WelcomePlayer` in native/Core source order.
///
/// # Safety
/// `level` and `connection` are live records owned by the selected backend;
/// `optional` is null or a readable NUL-terminated TCHAR string.
pub(crate) unsafe fn welcome_player_with<O: WelcomePlayerOps>(
    ops: &mut O,
    level: *mut c_void,
    connection: *mut c_void,
    optional: *const u32,
) -> std::result::Result<(), O::Error> {
    // SAFETY: the adapter implements operations for live backend records.
    unsafe {
        ops.copy_package_map(connection)?;
        ops.send_package_map(connection)?;
        let (name, lone) = ops.welcome_values(level)?;
        let mut message: Vec<u32> = "WELCOME LEVEL=".chars().map(u32::from).collect();
        message.extend(name);
        message.extend(" LONE=".chars().map(u32::from));
        append_integer(&mut message, lone);
        if !optional.is_null() && *optional != 0 {
            message.push(u32::from(b' '));
            let mut cursor = optional;
            while *cursor != 0 { message.push(*cursor); cursor = cursor.add(1); }
        }
        ops.connection_log(connection, &message)?;
        ops.flush_net(connection)?;
    }
    Ok(())
}

fn append_integer(out: &mut Vec<u32>, value: i32) {
    out.extend(value.to_string().chars().map(u32::from));
}

/// Direct Core implementation of `ULevel::WelcomePlayer`.
///
/// # Safety
/// `level` and `connection` are live canonical Core records; `optional` is null
/// or a readable NUL-terminated TCHAR string.
pub unsafe fn welcome_player(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    connection: *mut UNetConnection,
    optional: *const u32,
) -> RuntimeResult<()> {
    let mut ops = CoreWelcomeOps { access };
    unsafe { welcome_player_with(&mut ops, level.cast(), connection.cast(), optional) }
}

pub(crate) unsafe fn log_connection(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    message: &[u32],
) -> RuntimeResult<()> {
    if connection.is_null() { return Err(RuntimeError::new("Null network connection log target")); }
    let channel = unsafe { (*connection).channels[0].cast::<hp2_abi::engine::generated::UControlChannel>() };
    if !channel.is_null() && unsafe { (*channel).base.closing == 0 } {
        let mut text = message.to_vec();
        text.push(0);
        unsafe { crate::channel_owner::serialize_control(access, channel, text.as_ptr())?; }
    }
    Ok(())
}

struct CoreWelcomeOps<'a> { access: CoreAccess<'a> }

impl WelcomePlayerOps for CoreWelcomeOps<'_> {
    type Error = RuntimeError;

    unsafe fn copy_package_map(&mut self, connection: *mut c_void) -> RuntimeResult<()> {
        let conn = connection.cast::<UNetConnection>();
        let driver = unsafe { (*conn).driver.cast::<UNetDriver>() };
        let package_map = unsafe { (*conn).package_map.cast::<UPackageMap>() };
        let master_map = unsafe { (*driver).master_map.cast::<UPackageMap>() };
        unsafe { crate::package_map_level_owner::copy_linkers(self.access, package_map, master_map) }
    }

    unsafe fn send_package_map(&mut self, connection: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::net_conn::send_package_map_with(connection.cast(), &CoreSendPackageMapOps { access: self.access }) }
    }

    unsafe fn welcome_values(&mut self, level: *mut c_void) -> RuntimeResult<(Vec<u32>, i32)> {
        let level = level.cast::<ULevel>();
        let outer = unsafe { (*level).base.base.outer.cast::<UObject>() };
        if outer.is_null() { return Err(RuntimeError::new("ULevel has no outer")); }
        let name_index = i32::from_ne_bytes(unsafe { (*outer).name });
        let name = self.access.name_text(name_index)?;
        let info = unsafe { crate::package_map_level_owner::level_info(level)? };
        let lone = unsafe { (*info).b_lone_player() };
        Ok((name, i32::from(lone)))
    }

    unsafe fn connection_log(&mut self, connection: *mut c_void, message: &[u32]) -> RuntimeResult<()> {
        unsafe { log_connection(self.access, connection.cast(), message) }
    }

    unsafe fn flush_net(&mut self, connection: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::flush_net(self.access, connection.cast()) }
    }
}

struct CoreSendPackageMapOps<'a> { access: CoreAccess<'a> }

impl crate::net_conn::SendPackageMapOps for CoreSendPackageMapOps<'_> {
    type Error = RuntimeError;
    unsafe fn package_count(&self, connection: *mut UNetConnection) -> RuntimeResult<i32> {
        let map = unsafe { (*connection).package_map.cast::<UPackageMap>() };
        if map.is_null() { return Err(RuntimeError::new("Null Connection.PackageMap")); }
        let record = unsafe { &*map.cast::<hp2_core::core_records::PackageMapRecord>() };
        if record.list.num < 0 || record.list.max < record.list.num || (record.list.num > 0 && record.list.data.is_null()) {
            return Err(RuntimeError::new("Invalid PackageMap List storage"));
        }
        Ok(record.list.num)
    }
    unsafe fn log_package_use(&self, connection: *mut UNetConnection, index: i32) -> RuntimeResult<()> {
        let map = unsafe { (*connection).package_map.cast::<UPackageMap>() };
        let record = unsafe { &*map.cast::<hp2_core::core_records::PackageMapRecord>() };
        if index < 0 || index >= record.list.num || record.list.data.is_null() {
            return Err(RuntimeError::new("PackageMap list index is out of range"));
        }
        let info = unsafe { &*record.list.data.cast::<FPackageInfo>().add(index as usize) };
        let parent = info.parent.cast::<UObject>();
        if parent.is_null() { return Err(RuntimeError::new("PackageMap package has no parent")); }
        let package_name = self.access.name_text(i32::from_ne_bytes(unsafe { (*parent).name }))?;
        let filename = fstring_units(&info.url)?;
        let filename = filename.rsplit(|unit| *unit == u32::from(b'/') || *unit == u32::from(b'\\') || *unit == u32::from(b':')).next().unwrap_or_default();
        let guid = format!("{:08X}{:08X}{:08X}{:08X}", info.guid[0], info.guid[1], info.guid[2], info.guid[3]);
        let name: String = package_name.into_iter().filter_map(char::from_u32).collect();
        let file: String = filename.iter().copied().filter_map(char::from_u32).collect();
        let text = format!("USES GUID={guid} PKG={name} FLAGS={} SIZE={} GEN={} FNAME={file}", info.package_flags, info.file_size, info.local_generation);
        self.log(connection, &text)
    }
    unsafe fn download_manager_count(&self, driver: *mut UNetDriver) -> RuntimeResult<i32> {
        let managers = unsafe { (*driver).download_managers };
        if managers.num < 0 || managers.max < managers.num || (managers.num > 0 && managers.data.is_null()) {
            return Err(RuntimeError::new("Invalid DownloadManagers storage"));
        }
        Ok(managers.num)
    }
    unsafe fn load_download_class(&self, driver: *mut UNetDriver, index: i32) -> RuntimeResult<*mut c_void> {
        let managers = unsafe { (*driver).download_managers };
        let units = unsafe { fstring_units(&*managers.data.cast::<hp2_abi::engine::generated::TArrayRaw>().add(index as usize))? };
        let class_name = terminated(units);
        let base = self.access.native_class("Engine.Download")?;
        let class = unsafe { self.access.static_load_class(base, ptr::null_mut(), &class_name, 0x0002)? };
        Ok(class.cast())
    }
    unsafe fn log_download_defaults(&self, connection: *mut UNetConnection, class: *mut c_void) -> RuntimeResult<()> {
        let class = class.cast::<hp2_abi::engine::generated::UClass>();
        let defaults = unsafe { self.access.default_object(class)? }.cast::<UDownload>();
        let params_name: Vec<u32> = "DownloadParams".chars().map(u32::from).collect();
        let params = unsafe { self.access.export_property_text(defaults.cast(), &params_name)? };
        let compression = unsafe { (*defaults).use_compression };
        if params.iter().any(|unit| *unit != 0) {
            let full_name = unsafe { self.access.object_full_name(class.cast())? };
            let full_name = units_to_string(&full_name);
            let class_name = full_name.split_once(' ').map_or(full_name.as_str(), |(_, path)| path);
            let text = format!("DLMGR CLASS={class_name} PARAMS={} COMPRESSION={compression}", units_to_string(&params));
            self.log(connection, &text)?;
        }
        Ok(())
    }
}

impl CoreSendPackageMapOps<'_> {
    fn log(&self, connection: *mut UNetConnection, text: &str) -> RuntimeResult<()> {
        let channel = unsafe { (*connection).channels[0].cast::<hp2_abi::engine::generated::UControlChannel>() };
        if !channel.is_null() && unsafe { (*channel).base.closing == 0 } {
            let mut units: Vec<u32> = text.chars().map(u32::from).collect();
            units.push(0);
            unsafe { crate::channel_owner::serialize_control(self.access, channel, units.as_ptr())?; }
        }
        Ok(())
    }
}

fn fstring_units(value: &hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<&[u32]> {
    if value.num < 0 || (value.num > 0 && value.data.is_null()) { return Err(RuntimeError::new("Invalid FString storage")); }
    if value.num == 0 { return Ok(&[]); }
    let units = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) };
    Ok(if units.last() == Some(&0) { &units[..units.len() - 1] } else { units })
}
fn terminated(units: &[u32]) -> Vec<u32> { let mut value = units.to_vec(); value.push(0); value }
fn units_to_string(units: &[u32]) -> String { units.iter().copied().take_while(|unit| *unit != 0).filter_map(char::from_u32).collect() }

pub(crate) trait AcceptChannelOps {
    type Error;
    fn check(&mut self, valid: bool, expression: &std::ffi::CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn with_full_name<T>(
        &mut self,
        object: *mut UObject,
        body: impl FnOnce(&mut Self, *const u32) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;
    fn log(&mut self, message: &[u32]) -> Result<(), Self::Error>;
}

/// Applies the source channel-admission policy to native or canonical records.
///
/// # Safety
/// Non-null records and their connection/driver references must remain live
/// through logging and checked-failure callbacks.
pub(crate) unsafe fn notify_accepting_channel_with<O: AcceptChannelOps>(
    ops: &mut O,
    level: *mut ULevel,
    channel: *mut UChannel,
) -> Result<bool, O::Error> {
    use hp2_core::native_text::{TextPart as A, fmt};
    use crate::level_net::{CHTYPE_ACTOR, CHTYPE_CONTROL, CHTYPE_FILE};
    ops.check(!channel.is_null(), c"Channel", 1370)?;
    // SAFETY: the caller supplies live records; failed source checks stop execution.
    let connection = unsafe { (*channel).connection.cast::<UNetConnection>() };
    ops.check(!connection.is_null(), c"Channel->Connection", 1371)?;
    ops.check(
        !unsafe { (*connection).driver.is_null() },
        c"Channel->Connection->Driver",
        1372,
    )?;
    let driver = unsafe { (*connection).driver.cast::<UNetDriver>() };
    if !unsafe { (*driver).server_connection.is_null() } {
        if unsafe { (*channel).ch_type } == CHTYPE_ACTOR {
            return Ok(true);
        }
        ops.log(&fmt(&[
            A::L("Client refusing unwanted channel of type "),
            A::I(unsafe { (*channel).ch_type }),
        ]))?;
        return Ok(false);
    }
    let control = unsafe { (*channel).ch_index == 0 && (*channel).ch_type == CHTYPE_CONTROL };
    let file = !control && unsafe { (*channel).ch_type == CHTYPE_FILE };
    // SAFETY: the adapter keeps the name alive without copying native static text.
    unsafe { ops.with_full_name(level.cast(), |ops, name| {
        let message = if control {
            fmt(&[
                A::L("NotifyAcceptingChannel Control "), A::I((*channel).ch_index),
                A::L(" server "), A::P(name), A::L(": Accepted"),
            ])
        } else if file {
            fmt(&[
                A::L("NotifyAcceptingChannel File "), A::I((*channel).ch_index),
                A::L(" server "), A::P(name), A::L(": Accepted"),
            ])
        } else {
            fmt(&[
                A::L("NotifyAcceptingChannel "), A::I((*channel).ch_type),
                A::L(" "), A::I((*channel).ch_index),
                A::L(" server "), A::P(name), A::L(": Refused"),
            ])
        };
        ops.log(&message)?;
        Ok(control || file)
    }) }
}

struct CoreAcceptChannelOps<'a>(CoreAccess<'a>);

impl AcceptChannelOps for CoreAcceptChannelOps<'_> {
    type Error = RuntimeError;
    fn check(&mut self, valid: bool, expression: &std::ffi::CStr, line: i32) -> RuntimeResult<()> {
        if valid {
            Ok(())
        } else {
            Err(RuntimeError::new("Level channel-admission check failed")
                .with_context(&format!("UnLevel.cpp:{line}: {}", expression.to_string_lossy())))
        }
    }
    unsafe fn with_full_name<T>(
        &mut self,
        object: *mut UObject,
        body: impl FnOnce(&mut Self, *const u32) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        // SAFETY: the common policy passes a live object in this owner.
        let mut name = unsafe { self.0.object_full_name(object)? };
        if name.last() != Some(&0) { name.push(0); }
        body(self, name.as_ptr())
    }
    fn log(&mut self, message: &[u32]) -> RuntimeResult<()> {
        let category = self.0.intern_str("DevNet", hp2_core::name_owner::FindName::Add)?;
        self.0.emit_log(category, message)
    }
}

/// Accepts actor channels on clients, and initial control/file channels on servers.
///
/// # Safety
/// `level`, `channel` and its connection/driver are live canonical records.
pub unsafe fn notify_accepting_channel(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    channel: *mut UChannel,
) -> RuntimeResult<bool> {
    // SAFETY: this function preserves the caller's live canonical record contract.
    unsafe { notify_accepting_channel_with(&mut CoreAcceptChannelOps(access), level, channel) }
}

enum NotifyTarget {
    Level(*mut ULevel),
    NetPending(*mut hp2_abi::engine::generated::UNetPendingLevel),
    DemoPending(*mut hp2_abi::engine::generated::UDemoPlayPendingLevel),
}

unsafe fn notification_target(access: CoreAccess<'_>, driver: *mut UNetDriver) -> RuntimeResult<NotifyTarget> {
    if driver.is_null() {
        return Err(RuntimeError::new("Network notification has no driver"));
    }
    let notify = unsafe { (*driver).notify };
    if notify.is_null() {
        return Err(RuntimeError::new("Network driver has no notification target"));
    }
    // SAFETY: the caller supplies a canonical ULevelBase secondary FNetworkNotify
    // base. Its source ABI follows the UObject prefix, as in demo notify_get_level.
    let object = unsafe { notify.cast::<u8>().sub(std::mem::size_of::<UObject>()).cast::<UObject>() };
    let mut class = unsafe { (*object).class.cast::<UClass>() };
    let level = access.native_class("Engine.Level")?;
    let net = access.native_class("Engine.NetPendingLevel")?;
    let demo = access.native_class("Engine.DemoPlayPendingLevel")?;
    while !class.is_null() {
        if class == level { return Ok(NotifyTarget::Level(object.cast())); }
        if class == net { return Ok(NotifyTarget::NetPending(object.cast())); }
        if class == demo { return Ok(NotifyTarget::DemoPending(object.cast())); }
        class = unsafe { (*class).super_field.cast() };
    }
    Err(RuntimeError::new("Network driver notification target has no canonical implementation"))
}

/// Routes channel admission through the driver's actual Level/Pending identity.
///
/// # Safety
/// Driver, its secondary notification base and channel are live canonical records.
pub unsafe fn notify_accepting_channel_for_driver(
    access: CoreAccess<'_>,
    driver: *mut UNetDriver,
    channel: *mut UChannel,
) -> RuntimeResult<bool> {
    // SAFETY: notification_target validates the canonical class identity before dispatch.
    match unsafe { notification_target(access, driver)? } {
        NotifyTarget::Level(level) => unsafe { notify_accepting_channel(access, level, channel) },
        NotifyTarget::NetPending(_) => Ok(crate::net_pending::notify_accepting_channel() != 0),
        // UnDemoPenLev.h explicitly accepts channels while demo metadata loads.
        NotifyTarget::DemoPending(_) => Ok(true),
    }
}

/// Routes download progress to the canonical Engine, never a C++ notify vptr.
///
/// # Safety
/// The connection, driver, notification owner and its Engine graph are live
/// canonical records. Texts are readable NUL-terminated TCHAR strings.
pub unsafe fn notify_progress(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    message: *const u32,
    progress: *const u32,
    seconds: f32,
) -> RuntimeResult<()> {
    use hp2_abi::engine::generated::{UGameEngine, ULevelBase};
    let driver = unsafe { (*connection).driver.cast() };
    let owner: *mut ULevelBase = match unsafe { notification_target(access, driver)? } {
        NotifyTarget::Level(level) => level.cast(),
        NotifyTarget::NetPending(pending) => pending.cast(),
        NotifyTarget::DemoPending(pending) => pending.cast(),
    };
    let engine = unsafe { (*owner).engine.cast::<UGameEngine>() };
    let game_class = access.native_class("Engine.GameEngine")?;
    if unsafe { hp2_core::object_model::is_a(engine.cast(), game_class.cast()) } {
        return unsafe { crate::game_engine_owner::set_progress(access, engine, message, progress, seconds) };
    }
    // UEngine::SetProgress is intentionally empty. Only GameEngine overrides it
    // among the canonical registered Engine classes.
    Ok(())
}

struct CorePendingFileOps<'a>(CoreAccess<'a>);

impl crate::net_pending::PendingFileOps for CorePendingFileOps<'_> {
    type Error = RuntimeError;
    fn check(&mut self, valid: bool, expression: &std::ffi::CStr, line: i32) -> RuntimeResult<()> {
        if valid { Ok(()) } else {
            Err(RuntimeError::new("Pending download check failed")
                .with_context(&format!("UnPenLev.cpp:{line}: {}", expression.to_string_lossy())))
        }
    }
    unsafe fn remove_download_method(&mut self, connection: *mut c_void) -> RuntimeResult<()> {
        use hp2_abi::engine::generated::FDownloadInfo;
        let array = unsafe { ptr::addr_of_mut!((*connection.cast::<UNetConnection>()).download_info) };
        let first = unsafe { (*array).data.cast::<FDownloadInfo>() };
        unsafe {
            self.0.destroy_owned_fstring(ptr::addr_of_mut!((*first).class_name))?;
            self.0.destroy_owned_fstring(ptr::addr_of_mut!((*first).params))?;
            remove_array_item(array, 0, std::mem::size_of::<FDownloadInfo>())
        }
    }
    unsafe fn record_failure(&mut self, destination: *mut hp2_abi::engine::generated::TArrayRaw, parent: *mut c_void, error: *const u32) -> RuntimeResult<()> {
        use crate::net_download::{FormatArg, localized_format};
        let format = self.0.localize_text(
            hp2_core::lit!("Errors"), hp2_core::lit!("DownloadFailed"),
            Some(hp2_core::lit!("Engine")), None, false,
        )?;
        let name = self.0.name_text(i32::from_ne_bytes(unsafe { (*parent.cast::<UObject>()).name }))?;
        let name = units_to_string(&name);
        let mut length = 0;
        while unsafe { *error.add(length) } != 0 { length += 1; }
        let error = units_to_string(unsafe { std::slice::from_raw_parts(error, length) });
        let message = localized_format(&units_to_string(&format), &[FormatArg::Text(&name), FormatArg::Text(&error)]);
        unsafe { self.0.assign_fstring(destination, &message) }
    }
    unsafe fn remove_package(&mut self, map: *mut c_void, index: i32) -> RuntimeResult<()> {
        let array = unsafe { ptr::addr_of_mut!((*map.cast::<hp2_core::core_records::PackageMapRecord>()).list) };
        let info = unsafe { (*array).data.cast::<FPackageInfo>().add(index as usize) };
        unsafe {
            self.0.destroy_owned_fstring(ptr::addr_of_mut!((*info).url))?;
            remove_array_item(array, index, std::mem::size_of::<FPackageInfo>())
        }
    }
    unsafe fn request_file(&mut self, connection: *mut c_void, index: i32) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::receive_file(self.0, connection.cast(), index) }
    }
    unsafe fn delete_download(&mut self, download: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.0.delete_object(download.cast()) }
    }
}

unsafe fn remove_array_item(
    array: *mut hp2_abi::engine::generated::TArrayRaw, index: i32, width: usize,
) -> RuntimeResult<()> {
    use hp2_core::array_storage::{self, Hp2ArraySlots};
    let slots = unsafe { Hp2ArraySlots {
        data: ptr::addr_of_mut!((*array).data),
        num: ptr::addr_of_mut!((*array).num),
        max: ptr::addr_of_mut!((*array).max),
    } };
    unsafe { array_storage::remove(slots, index, 1, width as i32, ptr::null(), array_storage::system_allocator()) }
        .map_err(|_| RuntimeError::new("Unable to remove canonical pending download entry"))
}

/// Routes file completion to the driver's real canonical notification owner.
///
/// # Safety
/// Connection, driver, notification target and package index are live canonical
/// records. `error` is NUL-terminated and remains live through transfer callbacks.
pub unsafe fn notify_received_file(
    access: CoreAccess<'_>, connection: *mut UNetConnection, package_index: i32,
    error: *const u32, skipped: bool,
) -> RuntimeResult<()> {
    let driver = unsafe { (*connection).driver.cast() };
    match unsafe { notification_target(access, driver)? } {
        NotifyTarget::Level(_) => Err(RuntimeError::new("Level received unexpected file")),
        NotifyTarget::NetPending(pending) => unsafe {
            crate::net_pending::notify_received_file_with(
                &mut CorePendingFileOps(access), pending.cast(), connection.cast(),
                package_index, error, skipped,
            )
        },
        // UnDemoPenLev.h intentionally ignores received-file notifications.
        NotifyTarget::DemoPending(_) => Ok(()),
    }
}

pub(crate) fn file_send_admission<E>(
    server_connection: bool, log: impl FnOnce(&[u32]) -> Result<(), E>,
) -> Result<bool, E> {
    log(if server_connection {
        hp2_core::lit!("Server requested file: Refused")
    } else {
        hp2_core::lit!("Client requested file: Allowed")
    })?;
    Ok(!server_connection)
}

/// Uses the actual Level/Pending notification implementation for a file request.
///
/// # Safety
/// Connection, driver and the canonical secondary notification base remain live.
pub unsafe fn notify_sending_file(
    access: CoreAccess<'_>, connection: *mut UNetConnection, _guid: &[u32; 4],
) -> RuntimeResult<bool> {
    let driver = unsafe { (*connection).driver.cast() };
    match unsafe { notification_target(access, driver)? } {
        NotifyTarget::Level(level) => {
            let driver = unsafe { (*level).base.net_driver.cast::<UNetDriver>() };
            let server = unsafe { !(*driver).server_connection.is_null() };
            file_send_admission(server, |message| {
                let category = access.intern_str("DevNet", hp2_core::name_owner::FindName::Add)?;
                access.emit_log(category, message)
            })
        }
        NotifyTarget::NetPending(_) => {
            let message = access.localize_text(
                hp2_core::lit!("Errors"), hp2_core::lit!("RequestDenied"),
                Some(hp2_core::lit!("Engine")), None, false,
            )?;
            let category = access.intern_str("DevNet", hp2_core::name_owner::FindName::Add)?;
            access.emit_log(category, &message)?;
            Ok(false)
        }
        NotifyTarget::DemoPending(_) => Ok(false),
    }
}

/// Dispatches control-channel text to its actual canonical notification owner.
///
/// # Safety
/// The driver/connection, its secondary notification base and NUL-terminated
/// UTF-32 text remain live for the synchronous protocol/event callbacks.
pub unsafe fn notify_received_text_for_driver(
    access: CoreAccess<'_>, driver: *mut c_void,
    connection: *mut UNetConnection, text: *const u32,
) -> RuntimeResult<()> {
    match unsafe { notification_target(access, driver.cast())? } {
        NotifyTarget::Level(level) => unsafe {
            crate::level_notify_owner::notify_received_text(access, level, connection, text)
        },
        NotifyTarget::NetPending(pending) => unsafe {
            crate::pending_notify_owner::net_received_text(access, pending, connection, text)
        },
        NotifyTarget::DemoPending(pending) => unsafe {
            crate::pending_notify_owner::demo_received_text(access, pending, connection, text)
        },
    }
}
