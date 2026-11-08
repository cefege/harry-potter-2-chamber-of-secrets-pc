//! Canonical Engine file-channel lifetime and receive dispatch.

use crate::net_chan::ChannelTickOps;
use hp2_abi::engine::generated::{FInBunch, FOutBunch, FPackageInfo, UChannel, UFileChannel, UNetConnection, UNetDriver, UPackageMap, UObject};
use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_access::{CoreAccess, RuntimeError, RuntimeResult}, core_class_specs::{NativeClassSpec, NativeRecordKind}, object_owner::ClassOps};
use std::{alloc::Layout, cell::Cell, ffi::c_void, fs::{self, File}, io::Read, path::PathBuf, ptr};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_TRANSIENT: u32 = 0x0000_0008;

/// Canonical Engine FileChannel class declaration.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "FileChannel",
    package: "Engine",
    super_name: Some("Channel"),
    within: "Object",
    flags: CLASS_TRANSIENT,
    object_flags: OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<UFileChannel>()),
    operations: Some(FILE_CHANNEL_OPERATIONS),
    properties: &[],
    defaults: &[],
}];

const FILE_CHANNEL_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_file_channel;
    operations
};

/// Constructs a FileChannel without initialization or connection publication.
///
/// # Safety
/// Core must contain the canonical Engine.FileChannel class.
pub unsafe fn construct(access: CoreAccess<'_>) -> RuntimeResult<*mut UFileChannel> {
    let class = access.native_class("Engine.FileChannel")?;
    let object = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?
    };
    Ok(object.cast())
}

/// Constructs and initializes a file channel without publishing connection slots.
///
/// # Safety
/// `connection` must be live and `index` a valid channel slot.
pub unsafe fn create(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
) -> RuntimeResult<*mut UFileChannel> {
    if connection.is_null() {
        return Err(RuntimeError::new("Null canonical UNetConnection"));
    }
    if !(0..1023).contains(&index) {
        return Err(RuntimeError::new("File channel index is outside connection slots"));
    }
    let channel = unsafe { construct(access)? };
    if let Err(error) = unsafe { initialize(access, channel, connection, index, opened_locally) } {
        unsafe { access.delete_object(channel.cast())?; }
        return Err(error);
    }
    Ok(channel)
}

/// Initializes the shared channel and file-channel fields.
///
/// # Safety
/// `channel` and `connection` must be live canonical objects.
pub unsafe fn initialize(
    _access: CoreAccess<'_>,
    channel: *mut UFileChannel,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
) -> RuntimeResult<()> {
    if channel.is_null() || connection.is_null() {
        return Err(RuntimeError::new("Invalid file-channel initialization arguments"));
    }
    unsafe {
        crate::channel_owner::init_channel(channel.cast::<UChannel>(), connection, index, opened_locally)?;
        (*channel).download = ptr::null_mut();
        (*channel).send_file_ar = ptr::null_mut();
        (*channel).src_filename.fill(0);
        (*channel).package_index = 0;
        (*channel).sent_data = 0;
    }
    Ok(())
}

/// Deletes a canonical file channel after its close/ack policy is complete.
///
/// # Safety
/// `channel` must be a registered live file channel with intact connection slots.
pub unsafe fn delete(access: CoreAccess<'_>, channel: *mut UFileChannel) -> RuntimeResult<()> {
    if channel.is_null() {
        return Err(RuntimeError::new("Null canonical UFileChannel"));
    }
    unsafe { crate::channel_owner::delete_channel(access, channel.cast()) }
}

/// Processes one received file-channel bunch through the canonical receive policy.
///
/// # Safety
/// The channel and bunch, including their connection and package-map storage, must be live.
pub unsafe fn received_bunch(
    access: CoreAccess<'_>,
    channel: *mut UFileChannel,
    bunch: *mut FInBunch,
) -> RuntimeResult<()> {
    if channel.is_null() || bunch.is_null() {
        return Err(RuntimeError::new("Invalid file-channel bunch arguments"));
    }
    unsafe { crate::net_chan::file_received_bunch_core(access, channel, bunch) }
}

pub(crate) struct CoreFileReceiveOps<'a> {
    access: CoreAccess<'a>,
}

impl<'a> CoreFileReceiveOps<'a> {
    pub(crate) fn new(access: CoreAccess<'a>) -> Self { Self { access } }
}


fn tchar_text(units: &[u32]) -> String {
    units.iter().copied().take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER)).collect()
}

fn engine_file_path(access: CoreAccess<'_>, filename: &str) -> PathBuf {
    let path = PathBuf::from(filename);
    if path.is_absolute() { path } else { access.data_root().join("System").join(path) }
}

/// Returns `GFileManager->FileSize(SaveSlotPath / Save{slot}.usa)`, or -1 when absent.
pub fn save_slot_file_size(access: CoreAccess<'_>, slot: i32) -> RuntimeResult<i32> {
    let system = access.system()?;
    let raw = unsafe { ptr::addr_of!((*system).save_slot_path).read_unaligned() };
    if raw.num < 0 || (raw.num > 0 && raw.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical SaveSlotPath"));
    }
    let path_text = if raw.num > 0 {
        tchar_text(unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) })
    } else {
        String::new()
    };
    let mut path = engine_file_path(access, &path_text);
    path.push(format!("Save{slot}.usa"));
    Ok(fs::metadata(path)
        .ok()
        .and_then(|metadata| i32::try_from(metadata.len()).ok())
        .unwrap_or(-1))
}

pub(crate) unsafe fn package_info(connection: *mut UNetConnection, index: i32) -> RuntimeResult<*mut FPackageInfo> {
    if connection.is_null() || index < 0 { return Err(RuntimeError::new("Invalid file package index")); }
    let map = unsafe { (*connection).package_map.cast::<UPackageMap>() };
    if map.is_null() { return Err(RuntimeError::new("Connection has no package map")); }
    let list = unsafe { (*map).list };
    if index >= list.num || list.data.is_null() { return Err(RuntimeError::new("File package index is out of range")); }
    Ok(unsafe { list.data.cast::<FPackageInfo>().add(index as usize) })
}

impl crate::net_chan::FileReceiveOps for CoreFileReceiveOps<'_> {
    type Error = RuntimeError;
    unsafe fn check_fail(&mut self, expression: &'static std::ffi::CStr, line: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(&format!("{} at UnChan.cpp:{line}", expression.to_string_lossy())))
    }
    unsafe fn read_bunch_skip(&mut self, bunch: *mut FInBunch) -> RuntimeResult<(bool, bool)> {
        if bunch.is_null() { return Err(RuntimeError::new("Null file receive bunch")); }
        let mut text = Vec::new();
        let decoded = unsafe {
            crate::actor_channel_owner::with_core_bunch_archive(self.access, bunch, |archive| {
                archive.serialize_fstring(&mut text)
            })
        };
        if let Err(error) = decoded {
            if unsafe { (*bunch).base.ar_is_error != 0 } { return Ok((true, false)); }
            return Err(error);
        }
        let text = text.strip_suffix(&[0]).unwrap_or(&text);
        let skip = text == [b'S' as u32, b'K' as u32, b'I' as u32, b'P' as u32];
        Ok((unsafe { (*bunch).base.ar_is_error != 0 }, skip))
    }
    unsafe fn read_bunch_guid(&mut self, bunch: *mut FInBunch) -> RuntimeResult<([u32; 4], bool)> {
        let mut guid = [0u32; 4];
        let decoded = unsafe {
            crate::actor_channel_owner::with_core_bunch_archive(self.access, bunch, |archive| {
                for part in &mut guid { archive.serialize_u32(part)?; }
                Ok(())
            })
        };
        if let Err(error) = decoded {
            if unsafe { (*bunch).base.ar_is_error != 0 } { return Ok((guid, true)); }
            return Err(error);
        }
        Ok((guid, unsafe { (*bunch).base.ar_is_error != 0 }))
    }
    unsafe fn bunch_data(&mut self, bunch: *mut FInBunch) -> RuntimeResult<(*const u8, i32)> {
        if bunch.is_null() { return Err(RuntimeError::new("Null file receive bunch")); }
        let reader = unsafe { (*bunch).base };
        Ok((reader.buffer.data.cast(), reader.num.saturating_add(7) >> 3))
    }
    unsafe fn receive_download_data(&mut self, download: *mut hp2_abi::engine::generated::UDownload, data: *const u8, bytes: i32) -> RuntimeResult<()> {
        unsafe { crate::download_owner::receive_data(self.access, download, data, bytes) }
    }
    unsafe fn log_file_invalid(&mut self) -> RuntimeResult<()> {
        let text = crate::download_owner::localized_download_text(self.access, "Errors", "NetInvalid")?;
        crate::download_owner::emit_download_log(self.access, "DevNet", &text)
    }
    unsafe fn send_close_reply(&mut self, channel: *mut UChannel) -> RuntimeResult<()> {
        unsafe {
            crate::net_bunch::with_out_core(channel, 1, |reply| {
                crate::net_chan::channel_send_bunch_core(self.access, channel, reply, 0)
                    .map(|_| ()).map_err(|_| crate::net_chan::Thrown)
            })
        }.map_err(|_| RuntimeError::new("Unable to send file-channel close reply"))
    }
    unsafe fn package_count(&mut self, connection: *mut UNetConnection) -> RuntimeResult<i32> {
        if connection.is_null() { return Err(RuntimeError::new("Null file-channel connection")); }
        let map = unsafe { (*connection).package_map.cast::<UPackageMap>() };
        if map.is_null() { return Err(RuntimeError::new("Connection has no package map")); }
        let count = unsafe { (*map).list.num };
        if count < 0 { return Err(RuntimeError::new("Invalid package-map count")); }
        Ok(count)
    }
    unsafe fn package_info(&mut self, connection: *mut UNetConnection, index: i32) -> RuntimeResult<crate::net_chan::FilePackageInfo> {
        let info = unsafe { package_info(connection, index)? };
        let url = unsafe {
            crate::net_chan::FileReceiveUrl::from_header(ptr::addr_of_mut!((*info).url).cast())
        }.ok_or_else(|| RuntimeError::new("Invalid package URL storage"))?;
        Ok(crate::net_chan::FilePackageInfo { url, guid: unsafe { (*info).guid } })
    }
    unsafe fn package_has_linker(&mut self, connection: *mut UNetConnection, index: i32) -> RuntimeResult<bool> {
        Ok(!unsafe { (*package_info(connection, index)?).linker }.is_null())
    }
    unsafe fn remove_package(&mut self, connection: *mut UNetConnection, index: i32) -> RuntimeResult<()> {
        let map = unsafe { (*connection).package_map.cast::<UPackageMap>() };
        if map.is_null() { return Err(RuntimeError::new("Connection has no package map")); }
        let info = unsafe { package_info(connection, index)? };
        unsafe { self.access.destroy_owned_fstring(ptr::addr_of_mut!((*info).url))?; }
        let list = ptr::addr_of_mut!((*map).list);
        let slots = Hp2ArraySlots {
            data: ptr::addr_of_mut!((*list).data).cast(),
            num: ptr::addr_of_mut!((*list).num),
            max: ptr::addr_of_mut!((*list).max),
        };
        unsafe {
            array_storage::remove(slots, index, 1, std::mem::size_of::<FPackageInfo>() as i32, b"FPackageInfo\0".as_ptr().cast(), array_storage::system_allocator())
        }.map_err(|_| RuntimeError::new("Unable to remove skipped package metadata"))
    }
    unsafe fn set_source_filename(&mut self, channel: *mut UFileChannel, url: crate::net_chan::FileReceiveUrl, _index: i32) -> RuntimeResult<()> {
        if channel.is_null() { return Err(RuntimeError::new("Null file channel")); }
        let units = unsafe { url.as_slice() };
        let units = &units[..units.iter().position(|unit| *unit == 0).unwrap_or(units.len())];
        let capacity = unsafe { (*channel).src_filename.len() };
        let copied = units.len().min(capacity.saturating_sub(1));
        unsafe {
            (*channel).src_filename.fill(0);
            (&mut (*channel).src_filename)[..copied].copy_from_slice(&units[..copied]);
        }
        Ok(())
    }
    unsafe fn notify_sending_file(&mut self, connection: *mut UNetConnection, guid: &[u32; 4]) -> RuntimeResult<bool> {
        unsafe { crate::level_net_owner::notify_sending_file(self.access, connection, guid) }
    }
    unsafe fn file_size(&mut self, url: crate::net_chan::FileReceiveUrl) -> RuntimeResult<i32> {
        let path = engine_file_path(self.access, &tchar_text(unsafe { url.as_slice() }));
        Ok(fs::metadata(path).ok().and_then(|metadata| i32::try_from(metadata.len()).ok()).unwrap_or(-1))
    }
    unsafe fn create_file_reader(&mut self, channel: *mut UFileChannel) -> RuntimeResult<()> {
        let filename = tchar_text(unsafe { &(*channel).src_filename });
        let path = engine_file_path(self.access, &filename);
        let file = File::open(path).ok();
        unsafe { (*channel).send_file_ar = file.map(|file| Box::into_raw(Box::new(file)).cast()).unwrap_or(ptr::null_mut()); }
        Ok(())
    }
    unsafe fn log_file_skip(&mut self, channel: *mut UFileChannel) -> RuntimeResult<()> {
        let name = tchar_text(unsafe { &(*channel).src_filename });
        crate::download_owner::emit_download_log(self.access, "DevNet", &format!("User skipped download of '{name}'"))
    }
    unsafe fn log_file_send(&mut self, channel: *mut UFileChannel) -> RuntimeResult<()> {
        let name = tchar_text(unsafe { &(*channel).src_filename });
        let format = crate::download_owner::localized_download_text(self.access, "Progress", "NetSend")?;
        let message = crate::net_download::localized_format(&format, &[crate::net_download::FormatArg::Text(&name)]);
        crate::download_owner::emit_download_log(self.access, "DevNet", &tchar_text(&message))
    }
    unsafe fn download_allowed(&mut self, connection: *mut UNetConnection) -> RuntimeResult<bool> {
        if connection.is_null() { return Err(RuntimeError::new("Null file-channel connection")); }
        let driver = unsafe { (*connection).driver.cast::<UNetDriver>() };
        if driver.is_null() { return Err(RuntimeError::new("File-channel connection has no driver")); }
        Ok(unsafe { (*driver).allow_downloads != 0 })
    }
    unsafe fn sending_file(&mut self, channel: *mut UFileChannel) -> bool {
        !channel.is_null() && unsafe { !(*channel).send_file_ar.is_null() }
    }
}

struct CoreFileTickOps<'a> {
    access: CoreAccess<'a>,
    archive_error: Cell<bool>,
}

impl crate::net_chan::ChannelTickOps for CoreFileTickOps<'_> {
    type Error = RuntimeError;
    unsafe fn check_failed(&self, expression: &std::ffi::CStr, line: i32) -> RuntimeResult<()> {
        let message = format!("{} (UnChan.cpp:{line})", expression.to_string_lossy());
        Err(RuntimeError::new(&message))
    }
    unsafe fn driver_time_since(&self, driver: *mut UNetDriver, time: *const u8) -> RuntimeResult<f32> {
        unsafe {
            crate::net_chan::CoreChannelTickOps::new(self.access)
                .driver_time_since(driver, time)
        }
    }
    unsafe fn send_raw_bunch(&self, connection: *mut UNetConnection, bunch: *mut FOutBunch) -> RuntimeResult<()> {
        unsafe {
            crate::net_chan::CoreChannelTickOps::new(self.access)
                .send_raw_bunch(connection, bunch)
        }
    }
    unsafe fn connection_is_net_ready(&self, connection: *mut UNetConnection, saturate: i32) -> RuntimeResult<i32> {
        unsafe {
            crate::net_chan::CoreChannelTickOps::new(self.access)
                .connection_is_net_ready(connection, saturate)
        }
    }
    unsafe fn connection_out_num_bits(&self, connection: *mut UNetConnection) -> RuntimeResult<i32> {
        unsafe {
            crate::net_chan::CoreChannelTickOps::new(self.access)
                .connection_out_num_bits(connection)
        }
    }
}

impl crate::net_chan::FileTickOps for CoreFileTickOps<'_> {
    unsafe fn package_file_size(&self, connection: *mut UNetConnection, package_index: i32) -> RuntimeResult<i32> {
        let info = unsafe { package_info(connection, package_index)? };
        Ok(unsafe { (*info).file_size })
    }
    unsafe fn archive_serialize(&self, archive: *mut c_void, data: *mut u8, count: i32) -> RuntimeResult<()> {
        if archive.is_null() || count < 0 || (count > 0 && data.is_null()) {
            return Err(RuntimeError::new("Invalid Core file-send archive request"));
        }
        if count == 0 { return Ok(()); }
        let output = unsafe { std::slice::from_raw_parts_mut(data, count as usize) };
        output.fill(0);
        if unsafe { (&mut *archive.cast::<File>()).read_exact(output) }.is_err() {
            self.archive_error.set(true);
        }
        Ok(())
    }
    unsafe fn archive_is_error(&self, archive: *mut c_void) -> RuntimeResult<bool> {
        if archive.is_null() { return Err(RuntimeError::new("Null Core file-send archive")); }
        Ok(self.archive_error.get())
    }
    unsafe fn bunch_serialize_bytes(&self, bunch: *mut FOutBunch, data: *mut u8, count: i32) -> RuntimeResult<()> {
        if bunch.is_null() || count < 0 || (count > 0 && data.is_null()) {
            return Err(RuntimeError::new("Invalid Core file-send bunch request"));
        }
        unsafe { hp2_core::bits::writer_serialize(bunch.cast(), data, count); }
        Ok(())
    }
    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> RuntimeResult<bool> {
        if bunch.is_null() { return Err(RuntimeError::new("Null Core file-send bunch")); }
        Ok(unsafe { (*bunch).base.ar_is_error != 0 })
    }
    unsafe fn with_out_bunch<F>(&self, channel: *mut UChannel, close: i32, body: F) -> RuntimeResult<()>
    where F: FnOnce(*mut FOutBunch) -> RuntimeResult<()> {
        let mut body_error = None;
        let result = unsafe {
            crate::net_bunch::with_out_core(channel, close, |bunch| {
                match body(bunch) {
                    Ok(()) => Ok(()),
                    Err(error) => { body_error = Some(error); Err(crate::net_chan::Thrown) }
                }
            })
        };
        match result {
            Ok(()) => Ok(()),
            Err(_) => Err(body_error.unwrap_or_else(|| RuntimeError::new("Unable to construct Core file-send bunch"))),
        }
    }
    unsafe fn send_bunch(&self, channel: *mut UChannel, bunch: *mut FOutBunch) -> RuntimeResult<()> {
        unsafe { crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, 0)?; }
        Ok(())
    }
    unsafe fn flush_net(&self, connection: *mut UNetConnection) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::flush_net(self.access, connection) }
    }
    unsafe fn delete_archive(&self, archive: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::download_owner::delete_send_archive(self.access, archive) }
    }
}

/// Advances file sending through the same packet policy used by native FileChannel.
///
/// # Safety
/// `channel` must be a live registered canonical file channel.
pub unsafe fn tick(access: CoreAccess<'_>, channel: *mut UFileChannel, lan_play: i32) -> RuntimeResult<()> {
    if channel.is_null() { return Err(RuntimeError::new("Null canonical UFileChannel")); }
    let ops = CoreFileTickOps { access, archive_error: Cell::new(false) };
    unsafe { crate::net_chan::file_tick_with(channel, lan_play, &ops) }
}

unsafe fn destroy_file_channel(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical file channel"));
    }
    let channel = object.cast::<UFileChannel>();
    unsafe {
        if !(*channel).send_file_ar.is_null() {
            crate::download_owner::delete_send_archive(access, (*channel).send_file_ar)?;
            (*channel).send_file_ar = ptr::null_mut();
        }
        if (*channel.cast::<UChannel>()).opened_locally != 0 && !(*channel).download.is_null() {
            crate::download_owner::download_done(access, (*channel).download.cast())?;
            crate::download_owner::delete_channel_download(access, (*channel).download.cast())?;
            (*channel).download = ptr::null_mut();
        }
    }
    crate::channel_owner::destroy_channel_body(access, channel.cast())
}
