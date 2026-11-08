//! Canonical Engine download and channel-download ownership.

use hp2_abi::engine::generated::{FPackageInfo, UChannelDownload, UDownload, UNetConnection, UObject};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, core_class_specs::NativeClassSpec, object_owner::ClassOps};
use std::{alloc::Layout, fs::{self, File, OpenOptions}, io::Write, path::PathBuf, ptr};

const OBJECT: ClassOps = hp2_core::core_class_specs::NativeRecordKind::Object.operations();
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_TRANSIENT: u32 = 0x0000_0008;
const CLASS_CONFIG: u32 = 0x0000_0004;
const CLASS_ABSTRACT: u32 = 0x0000_0001;

/// Canonical Engine Download and ChannelDownload class declarations.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Download", package: "Engine", super_name: Some("Object"), within: "Object",
        flags: CLASS_TRANSIENT | CLASS_CONFIG | CLASS_ABSTRACT, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: hp2_core::core_class_specs::NativeRecordKind::Object,
        layout: Some(Layout::new::<UDownload>()), operations: Some(DOWNLOAD_OPERATIONS),
        properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "ChannelDownload", package: "Engine", super_name: Some("Download"), within: "Object",
        flags: CLASS_TRANSIENT | CLASS_CONFIG, object_flags: OBJECT_FLAGS, config: "System", guid: [0; 4],
        kind: hp2_core::core_class_specs::NativeRecordKind::Object,
        layout: Some(Layout::new::<UChannelDownload>()), operations: Some(CHANNEL_DOWNLOAD_OPERATIONS),
        properties: &[], defaults: &[],
    },
];

const DOWNLOAD_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_download_object;
    operations
};
const CHANNEL_DOWNLOAD_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_channel_download_object;
    operations
};

/// Constructs the per-file channel download object owned by a file channel.
///
/// # Safety
/// `channel` must be a live canonical `UFileChannel`.
pub unsafe fn create_channel_download(access: CoreAccess<'_>, channel: *mut hp2_abi::engine::generated::UFileChannel) -> RuntimeResult<*mut UChannelDownload> {
    if channel.is_null() { return Err(RuntimeError::new("Null canonical UFileChannel")); }
    let class = access.native_class("Engine.ChannelDownload")?;
    let object = unsafe { access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())? }.cast::<UChannelDownload>();
    unsafe { (*object).ch = channel.cast(); }
    Ok(object)
}
/// Constructs a download instance from its method-selected reflected class.
///
/// # Safety
/// `class` must be a live canonical `UClass` derived from `UDownload`.
pub unsafe fn construct(
    access: CoreAccess<'_>,
    class: *mut hp2_abi::engine::generated::UClass,
) -> RuntimeResult<*mut UDownload> {
    if class.is_null() { return Err(RuntimeError::new("Null download class")); }
    let object = unsafe {
        access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?
    };
    Ok(object.cast())
}

/// Constructs the download class selected by the connection's first download method.
///
/// # Safety
/// `connection` must be a live canonical connection with a valid first download method.
pub unsafe fn construct_from_first_method(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
) -> RuntimeResult<*mut UDownload> {
    if connection.is_null() { return Err(RuntimeError::new("Null canonical UNetConnection")); }
    let methods = unsafe { (*connection).download_info };
    if methods.num <= 0 || methods.data.is_null() {
        return Err(RuntimeError::new("Connection has no first download method"));
    }
    let selected = unsafe { methods.data.cast::<hp2_abi::engine::generated::FDownloadInfo>().read().class };
    if selected.is_null() { return Err(RuntimeError::new("First download method has no class")); }
    unsafe { construct(access, selected.cast()) }
}

/// Constructs a base Engine download for direct API callers.
///
/// # Safety
/// `connection` must be live and `package_index` must identify an entry in its package map.
pub unsafe fn create(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
    package_index: i32,
) -> RuntimeResult<*mut UDownload> {
    let class = access.native_class("Engine.Download")?.cast();
    let object = unsafe { construct(access, class)? };
    if let Err(error) = unsafe { receive_file(access, object, connection, package_index) } {
        unsafe { access.delete_object(object.cast())?; }
        return Err(error);
    }
    Ok(object)
}

/// Dispatches ReceiveFile to the canonical subtype policy after connection publication.
///
/// # Safety
/// `download` and `connection` must be live canonical objects; `params` is a NUL-terminated TCHAR string.
/// Dispatches ReceiveFile after publication, preserving the original virtual's full arguments.
///
/// # Safety
/// `download` and `connection` must be live canonical objects; `params` is NUL-terminated TCHAR.
pub unsafe fn receive_file_dispatch(
    access: CoreAccess<'_>,
    download: *mut UDownload,
    connection: *mut UNetConnection,
    package_index: i32,
    params: *const u32,
    compression: bool,
) -> RuntimeResult<()> {
    if download.is_null() { return Err(RuntimeError::new("Null canonical UDownload")); }
    let channel_download_class = access.native_class("Engine.ChannelDownload")?;
    let actual_class = unsafe { (*download.cast::<UObject>()).class };
    let channel_download = unsafe {
        hp2_core::object_model::is_child_of(actual_class.cast(), channel_download_class.cast())
    };
    if channel_download {
        unsafe { channel_receive_file(access, download.cast(), connection, package_index, params, compression) }
    } else {
        unsafe { receive_file_with_options(access, download, connection, package_index, params, compression) }
    }
}



/// Associates a canonical download with the requested package and connection.
/// Creates the file channel and sends the package GUID for a channel download.
///
/// # Safety
/// `download` and `connection` must be live canonical objects; the package index must be valid.
/// Adapter for the subtype-specific file-channel construction and GUID send.
pub(crate) trait ChannelReceiveOps {
    type Error;
    unsafe fn receive_base(
        &mut self,
        download: *mut UChannelDownload,
        connection: *mut UNetConnection,
        package_index: i32,
        params: *const u32,
        compression: bool,
    ) -> Result<(), Self::Error>;
    unsafe fn create_file_channel(
        &mut self,
        connection: *mut UNetConnection,
    ) -> Result<*mut hp2_abi::engine::generated::UChannel, Self::Error>;
    unsafe fn attach(
        &mut self,
        channel: *mut hp2_abi::engine::generated::UChannel,
        download: *mut UChannelDownload,
        package_index: i32,
    ) -> Result<(), Self::Error>;
    unsafe fn allocation_failed(&mut self, download: *mut UChannelDownload) -> Result<(), Self::Error>;
    unsafe fn send_guid(
        &mut self,
        channel: *mut hp2_abi::engine::generated::UChannel,
        download: *mut UChannelDownload,
    ) -> Result<(), Self::Error>;
}

/// Shared ChannelDownload::ReceiveFile state machine.
///
/// # Safety
/// `download` and `connection` must be live canonical objects.
pub(crate) unsafe fn channel_receive_file_with<O: ChannelReceiveOps>(
    ops: &mut O,
    download: *mut UChannelDownload,
    connection: *mut UNetConnection,
    package_index: i32,
    params: *const u32,
    compression: bool,
) -> Result<(), O::Error> {
    unsafe { ops.receive_base(download, connection, package_index, params, compression)?; }
    let channel = unsafe { ops.create_file_channel(connection)? };
    if channel.is_null() {
        unsafe { ops.allocation_failed(download)?; }
        return Ok(());
    }
    unsafe {
        ops.attach(channel, download, package_index)?;
        ops.send_guid(channel, download)
    }
}

struct CoreChannelReceiveOps<'a> {
    access: CoreAccess<'a>,
}

pub unsafe fn channel_receive_file(
    access: CoreAccess<'_>,
    download: *mut UChannelDownload,
    connection: *mut UNetConnection,
    package_index: i32,
    params: *const u32,
    compression: bool,
) -> RuntimeResult<()> {
    let mut ops = CoreChannelReceiveOps { access };
    unsafe { channel_receive_file_with(&mut ops, download, connection, package_index, params, compression) }
}


impl ChannelReceiveOps for CoreChannelReceiveOps<'_> {
    type Error = RuntimeError;

    unsafe fn receive_base(&mut self, download: *mut UChannelDownload, connection: *mut UNetConnection, package_index: i32, params: *const u32, compression: bool) -> RuntimeResult<()> {
        unsafe { receive_file_with_options(self.access, download.cast(), connection, package_index, params, compression) }
    }
    unsafe fn create_file_channel(&mut self, connection: *mut UNetConnection) -> RuntimeResult<*mut hp2_abi::engine::generated::UChannel> {
        unsafe { crate::connection_owner::create_channel(self.access, connection, crate::level_net::CHTYPE_FILE, 1, -1) }
    }
    unsafe fn attach(&mut self, channel: *mut hp2_abi::engine::generated::UChannel, download: *mut UChannelDownload, package_index: i32) -> RuntimeResult<()> {
        unsafe {
            (*download).ch = channel.cast();
            let file_channel = channel.cast::<hp2_abi::engine::generated::UFileChannel>();
            (*file_channel).download = download.cast();
            (*file_channel).package_index = package_index;
        }
        Ok(())
    }
    unsafe fn allocation_failed(&mut self, download: *mut UChannelDownload) -> RuntimeResult<()> {
        assign_localized_error(self.access, download.cast(), "ChAllocate")?;
        unsafe { download_done(self.access, download.cast()) }
    }
    unsafe fn send_guid(&mut self, channel: *mut hp2_abi::engine::generated::UChannel, download: *mut UChannelDownload) -> RuntimeResult<()> {
        let info = unsafe { (*download.cast::<UDownload>()).info.cast::<FPackageInfo>() };
        if info.is_null() { return Err(RuntimeError::new("Channel download has no package metadata")); }
        unsafe {
            crate::net_bunch::with_out_core(channel, 0, |bunch| {
                let guid = ptr::addr_of!((*info).guid).cast::<u8>();
                hp2_core::bits::writer_serialize_bits(bunch.cast(), guid, 128);
                (*bunch).b_reliable = 1;
                if (*bunch).base.ar_is_error != 0 { return Err(crate::net_chan::Thrown); }
                crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, 0)
                    .map(|_| ()).map_err(|_| crate::net_chan::Thrown)
            })
        }.map_err(|_| RuntimeError::new("Unable to send the channel download GUID"))
    }
}

pub(crate) trait ChannelSkipOps {
    type Error;
    unsafe fn channel(&mut self, download: *mut UChannelDownload) -> Result<*mut hp2_abi::engine::generated::UChannel, Self::Error>;
    unsafe fn try_skip(&mut self, download: *mut UChannelDownload) -> Result<i32, Self::Error>;
    unsafe fn send_skip(&mut self, channel: *mut hp2_abi::engine::generated::UChannel) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn channel_try_skip_with<O: ChannelSkipOps>(
    ops: &mut O,
    download: *mut UChannelDownload,
) -> Result<i32, O::Error> {
    let channel = unsafe { ops.channel(download)? };
    if channel.is_null() { return Ok(0); }
    let skipped = unsafe { ops.try_skip(download)? };
    if skipped == 0 { return Ok(0); }
    unsafe { ops.send_skip(channel)?; }
    Ok(skipped)
}

struct CoreChannelSkipOps<'a> {
    access: CoreAccess<'a>,
}

pub unsafe fn channel_try_skip(access: CoreAccess<'_>, download: *mut UChannelDownload) -> RuntimeResult<i32> {
    let mut ops = CoreChannelSkipOps { access };
    unsafe { channel_try_skip_with(&mut ops, download) }
}


impl ChannelSkipOps for CoreChannelSkipOps<'_> {
    type Error = RuntimeError;
    unsafe fn channel(&mut self, download: *mut UChannelDownload) -> RuntimeResult<*mut hp2_abi::engine::generated::UChannel> {
        if download.is_null() { return Err(RuntimeError::new("Null canonical UChannelDownload")); }
        Ok(unsafe { (*download).ch.cast() })
    }
    unsafe fn try_skip(&mut self, download: *mut UChannelDownload) -> RuntimeResult<i32> {
        unsafe { try_skip(self.access, download.cast()) }
    }
    unsafe fn send_skip(&mut self, channel: *mut hp2_abi::engine::generated::UChannel) -> RuntimeResult<()> {
        unsafe {
            crate::net_bunch::with_out_core(channel, 1, |bunch| {
                hp2_core::compact_index::encode(5, |byte| {
                    hp2_core::bits::writer_serialize_bits(bunch.cast(), &byte, 8);
                    Ok::<u8, crate::net_chan::Thrown>(byte)
                })?;
                hp2_core::bits::writer_serialize(bunch.cast(), b"SKIP\0".as_ptr(), 5);

                (*bunch).b_reliable = 1;
                if (*bunch).base.ar_is_error != 0 { return Err(crate::net_chan::Thrown); }
                crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, 0)
                    .map(|_| ()).map_err(|_| crate::net_chan::Thrown)
            })
        }.map_err(|_| RuntimeError::new("Unable to send the channel download skip"))
    }
}

/// Requests an optional file skip over its existing file channel.
///
/// # Safety
/// `download` is live and any attached channel is a live canonical channel.
/// # Safety
/// `download` and `connection` must be live; `package_index` indexes the connection package map.
pub(crate) trait ReceiveFileOps {
    type Error;
    unsafe fn resolve_info(
        &mut self,
        connection: *mut UNetConnection,
        package_index: i32,
        params: *const u32,
        compression: bool,
    ) -> Result<*mut FPackageInfo, Self::Error>;
    fn invalid_request(&mut self) -> Self::Error;
}

pub(crate) unsafe fn receive_file_with<O: ReceiveFileOps>(
    ops: &mut O,
    download: *mut UDownload,
    connection: *mut UNetConnection,
    package_index: i32,
    params: *const u32,
    compression: bool,
) -> Result<(), O::Error> {
    if download.is_null() || connection.is_null() || package_index < 0 {
        return Err(ops.invalid_request());
    }
    let info = unsafe { ops.resolve_info(connection, package_index, params, compression)? };
    if info.is_null() { return Err(ops.invalid_request()); }
    unsafe {
        (*download).connection = connection.cast();
        (*download).package_index = package_index;
        (*download).info = info.cast();
    }
    Ok(())
}


struct CoreReceiveFileOps;
impl ReceiveFileOps for CoreReceiveFileOps {
    type Error = RuntimeError;

    unsafe fn resolve_info(
        &mut self,
        connection: *mut UNetConnection,
        package_index: i32,
        _params: *const u32,
        _compression: bool,
    ) -> RuntimeResult<*mut FPackageInfo> {
        // UDownload's native ReceiveFile policy does not inspect Params; Compression
        // is consumed by the shared state update after package-map resolution.
        let map = unsafe { (*connection).package_map.cast::<hp2_abi::engine::generated::UPackageMap>() };
        if map.is_null() { return Err(RuntimeError::new("Download connection has no package map")); }
        let list = unsafe { (*map).list };
        if package_index >= list.num || list.data.is_null() {
            return Err(RuntimeError::new("Download package index is out of range"));
        }
        Ok(unsafe { list.data.cast::<FPackageInfo>().add(package_index as usize) })
    }

    fn invalid_request(&mut self) -> RuntimeError {
        RuntimeError::new("Invalid canonical download request")
    }
}

pub unsafe fn receive_file(
    access: CoreAccess<'_>,
    download: *mut UDownload,
    connection: *mut UNetConnection,
    package_index: i32,
) -> RuntimeResult<()> {
    unsafe { receive_file_with_options(access, download, connection, package_index, ptr::null(), false) }
}

pub unsafe fn receive_file_with_options(
    _access: CoreAccess<'_>,
    download: *mut UDownload,
    connection: *mut UNetConnection,
    package_index: i32,
    params: *const u32,
    compression: bool,
) -> RuntimeResult<()> {
    unsafe { receive_file_with(&mut CoreReceiveFileOps, download, connection, package_index, params, compression) }
}

pub(crate) fn skip_eligible(file_open: bool, package_flags: u32) -> bool {
    file_open && package_flags & crate::net_download::PKG_CLIENT_OPTIONAL != 0
}

pub(crate) unsafe fn copy_error(download: *mut UDownload, message: *const u32) -> RuntimeResult<()> {
    if download.is_null() || message.is_null() {
        return Err(RuntimeError::new("Invalid download error arguments"));
    }
    let target = unsafe { &mut (*download).error };
    let mut index = 0;
    while index + 1 < target.len() && unsafe { *message.add(index) } != 0 {
        target[index] = unsafe { *message.add(index) };
        index += 1;
    }
    target[index] = 0;
    Ok(())
}
pub(crate) fn receive_progress(
    name: &str,
    transferred: i32,
    size: i32,
    receive_format: &str,
    size_format: &str,
) -> (Vec<u32>, Vec<u32>) {
    let message = crate::net_download::localized_format(
        receive_format,
        &[crate::net_download::FormatArg::Text(name)],
    );
    let percent = 100.0f32 * transferred as f32 / size as f32;
    let progress = crate::net_download::localized_format(
        size_format,
        &[
            crate::net_download::FormatArg::Int(size / 1024),
            crate::net_download::FormatArg::Float(percent as f64),
        ],
    );
    (message, progress)
}

/// Appends received bytes to the canonical cache temporary file and reports progress.
///
/// # Safety
/// `download` is live and `data` spans `count` readable bytes.
pub(crate) trait DownloadOps {
    type Error;

    unsafe fn file_is_open(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn transferred(&mut self, download: *mut UDownload) -> Result<i32, Self::Error>;
    unsafe fn prepare_receive(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn write_receive_data(
        &mut self,
        download: *mut UDownload,
        data: *const u8,
        count: i32,
    ) -> Result<bool, Self::Error>;
    unsafe fn add_transferred(&mut self, download: *mut UDownload, count: i32) -> Result<(), Self::Error>;
    unsafe fn open_error(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn write_error(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn report_progress(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    fn invalid_data(&mut self) -> Self::Error;
}

pub(crate) unsafe fn receive_data_with<O: DownloadOps>(
    ops: &mut O,
    download: *mut UDownload,
    data: *const u8,
    count: i32,
) -> Result<(), O::Error> {
    if download.is_null() || count < 0 || (count > 0 && data.is_null()) {
        return Err(ops.invalid_data());
    }
    if unsafe { ops.transferred(download)? == 0 && !ops.file_is_open(download)? } {
        unsafe { ops.prepare_receive(download)?; }
    }
    if !unsafe { ops.file_is_open(download)? } {
        unsafe { ops.open_error(download)?; }
        return Ok(());
    }
    if !unsafe { ops.write_receive_data(download, data, count)? } {
        unsafe { ops.write_error(download)?; }
        return Ok(());
    }
    unsafe { ops.add_transferred(download, count)?; }
    unsafe { ops.report_progress(download) }
}

pub unsafe fn receive_data(
    access: CoreAccess<'_>,
    download: *mut UDownload,
    data: *const u8,
    count: i32,
) -> RuntimeResult<()> {
    let mut ops = CoreDownloadOps { access };
    unsafe { receive_data_with(&mut ops, download, data, count) }
}

struct CoreDownloadOps<'a> {
    access: CoreAccess<'a>,
}

impl DownloadOps for CoreDownloadOps<'_> {
    type Error = RuntimeError;

    unsafe fn file_is_open(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        Ok(unsafe { !(*download).recv_file_ar.is_null() })
    }

    unsafe fn transferred(&mut self, download: *mut UDownload) -> RuntimeResult<i32> {
        Ok(unsafe { (*download).transfered })
    }

    unsafe fn prepare_receive(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let info = unsafe { (*download).info.cast::<FPackageInfo>() };
        if info.is_null() { return Err(RuntimeError::new("Download has no package metadata")); }
        let name = unsafe { package_name(self.access, info)? };
        emit_download_log(self.access, "DevNet", &format!("Receiving package '{name}'"))?;
        let (path, file) = loop {
            let path = unsafe { temp_path(self.access, download)? };
            if let Some(parent) = path.parent() {
                if fs::create_dir_all(parent).is_err() { return Ok(()); }
            }
            match OpenOptions::new().create_new(true).write(true).open(&path) {
                Ok(file) => break (path, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Ok(()),
            }
        };
        let owned = Box::into_raw(Box::new(file));
        let filename = path.to_string_lossy().chars().map(u32::from).chain([0]).collect::<Vec<_>>();
        if filename.len() > unsafe { (*download).temp_filename.len() } {
            unsafe { drop(Box::from_raw(owned)); }
            let _ = fs::remove_file(&path);
            return Ok(());
        }
        unsafe {
            (*download).temp_filename.fill(0);
            (&mut (*download).temp_filename)[..filename.len()].copy_from_slice(&filename);
            (*download).recv_file_ar = owned.cast();
        }
        Ok(())
    }

    unsafe fn write_receive_data(&mut self, download: *mut UDownload, data: *const u8, count: i32) -> RuntimeResult<bool> {
        if count > 0 {
            let bytes = unsafe { std::slice::from_raw_parts(data, count as usize) };
            let file = unsafe { &mut *(*download).recv_file_ar.cast::<File>() };
            return Ok(file.write_all(bytes).is_ok());
        }
        Ok(true)
    }

    unsafe fn add_transferred(&mut self, download: *mut UDownload, count: i32) -> RuntimeResult<()> {
        unsafe { (*download).transfered = (*download).transfered.wrapping_add(count); }
        Ok(())
    }

    unsafe fn open_error(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        assign_localized_error(self.access, download, "NetOpen")
    }

    unsafe fn write_error(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let path = unsafe { temp_path(self.access, download)? };
        assign_localized_error_with_args(
            self.access,
            download,
            "NetWrite",
            &[crate::net_download::FormatArg::Text(&path.to_string_lossy())],
        )
    }

    unsafe fn report_progress(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let info = unsafe { (*download).info.cast::<FPackageInfo>() };
        if info.is_null() { return Err(RuntimeError::new("Download has no package metadata")); }
        let flags = unsafe { (*info).package_flags };
        let name = unsafe { package_name(self.access, info)? };
        let key = if flags & crate::net_download::PKG_CLIENT_OPTIONAL != 0 {
            "ReceiveOptionalFile"
        } else {
            "ReceiveFile"
        };
        let message_format = localized_download_text(self.access, "Progress", key)?;
        let size = unsafe { (*info).file_size };
        let size_format = localized_download_text(self.access, "Progress", "ReceiveSize")?;
        let (message, progress) = receive_progress(
            &name,
            unsafe { (*download).transfered },
            size,
            &message_format,
            &size_format,
        );
        unsafe {
            crate::level_net_owner::notify_progress(
                self.access, (*download).connection.cast(), message.as_ptr(), progress.as_ptr(), 4.0,
            )
        }
    }

    fn invalid_data(&mut self) -> RuntimeError {
        RuntimeError::new("Invalid download data")
    }
}
pub(crate) trait DownloadCompletionOps {
    type Error;

    unsafe fn close_receive_file(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn skipped(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn package_name(&mut self, download: *mut UDownload) -> Result<String, Self::Error>;
    unsafe fn log_skipped(&mut self, download: *mut UDownload, name: &str) -> Result<(), Self::Error>;
    unsafe fn remove_temp(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn progress(&mut self, download: *mut UDownload, message: &str) -> Result<(), Self::Error>;
    unsafe fn notify_received(&mut self, download: *mut UDownload, skipped: bool) -> Result<(), Self::Error>;
    unsafe fn transferred(&mut self, download: *mut UDownload) -> Result<i32, Self::Error>;
    unsafe fn has_error(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn set_error(&mut self, download: *mut UDownload, key: &'static str, name: Option<&str>) -> Result<(), Self::Error>;
    unsafe fn is_compressed(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn decompress(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn size_matches(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn move_to_cache(&mut self, download: *mut UDownload) -> Result<bool, Self::Error>;
    unsafe fn update_cache(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn flush_cache(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn download_done_with<O: DownloadCompletionOps>(
    ops: &mut O,
    download: *mut UDownload,
) -> Result<(), O::Error> {
    unsafe { ops.close_receive_file(download)?; }
    if unsafe { ops.skipped(download)? } {
        let name = unsafe { ops.package_name(download)? };
        unsafe {
            ops.log_skipped(download, &name)?;
            ops.remove_temp(download)?;
            ops.progress(download, &format!("Skipped '{name}'"))?;
            ops.notify_received(download, true)?;
        }
        return Ok(());
    }
    let name = unsafe { ops.package_name(download)? };
    if !unsafe { ops.has_error(download)? } && unsafe { ops.transferred(download)? == 0 } {
        unsafe { ops.set_error(download, "NetRefused", Some(&name))?; }
    }
    if !unsafe { ops.has_error(download)? } && unsafe { ops.is_compressed(download)? } {
        if !unsafe { ops.decompress(download)? } {
            unsafe { ops.set_error(download, "NetSize", None)?; }
        }
    }
    if !unsafe { ops.has_error(download)? } && !unsafe { ops.size_matches(download)? } {
        unsafe { ops.set_error(download, "NetSize", None)?; }
    }
    if !unsafe { ops.has_error(download)? } && !unsafe { ops.move_to_cache(download)? } {
        unsafe { ops.set_error(download, "NetMove", None)?; }
    }
    if unsafe { ops.has_error(download)? } {
        unsafe {
            ops.remove_temp(download)?;
            ops.notify_received(download, false)?;
        }
        return Ok(());
    }
    if let Err(error) = unsafe { ops.update_cache(download) } {
        let _ = unsafe { ops.flush_cache(download) };
        return Err(error);
    }
    if let Err(error) = unsafe { ops.progress(download, &format!("Received '{name}'")) } {
        let _ = unsafe { ops.flush_cache(download) };
        return Err(error);
    }
    if let Err(error) = unsafe { ops.notify_received(download, false) } {
        let _ = unsafe { ops.flush_cache(download) };
        return Err(error);
    }
    unsafe { ops.flush_cache(download) }
}

pub(crate) fn localized_download_text(
    access: CoreAccess<'_>,
    section: &str,
    key: &str,
) -> RuntimeResult<String> {
    let section = wide(section);
    let key = wide(key);
    let package = wide("Engine");
    let localized = access.localize_text(&section, &key, Some(&package), None, false)?;
    Ok(localized
        .into_iter()
        .take_while(|unit| *unit != 0)
        .filter_map(char::from_u32)
        .collect())
}

fn wide(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain([0]).collect()
}

unsafe fn package_name(access: CoreAccess<'_>, info: *mut FPackageInfo) -> RuntimeResult<String> {
    let parent = unsafe { (*info).parent.cast::<UObject>() };
    if parent.is_null() {
        return Err(RuntimeError::new("Downloaded package has no parent object"));
    }
    let name = access.name_text(i32::from_ne_bytes(unsafe { (*parent).name }))?;
    Ok(name.into_iter().filter_map(char::from_u32).collect())
}

pub(crate) fn emit_download_log(access: CoreAccess<'_>, category: &str, text: &str) -> RuntimeResult<()> {
    let category = access.intern_str(category, hp2_core::name_owner::FindName::Add)?;
    let message = text.chars().map(u32::from).collect::<Vec<_>>();
    access.emit_log(category, &message)
}

fn assign_localized_error(
    access: CoreAccess<'_>,
    download: *mut UDownload,
    key: &str,
) -> RuntimeResult<()> {
    assign_localized_error_with_args(access, download, key, &[])
}

fn assign_localized_error_with_args(
    access: CoreAccess<'_>,
    download: *mut UDownload,
    key: &str,
    args: &[crate::net_download::FormatArg<'_>],
) -> RuntimeResult<()> {
    let format = localized_download_text(access, "Errors", key)?;
    let message = crate::net_download::localized_format(&format, args);
    let target = unsafe { &mut (*download).error };
    target.fill(0);
    let copied = message.len().min(target.len() - 1);
    target[..copied].copy_from_slice(&message[..copied]);
    target[copied] = 0;
    Ok(())
}

/// Records an error message on the download object.
///
/// # Safety
/// `download` is live and `message` is NUL-terminated UTF-32.
pub unsafe fn download_error(_access: CoreAccess<'_>, download: *mut UDownload, message: *const u32) -> RuntimeResult<()> {
    unsafe { copy_error(download, message) }
}

/// Marks a currently receiving optional package as skipped.
///
/// # Safety
/// `download` is live.
pub unsafe fn try_skip(access: CoreAccess<'_>, download: *mut UDownload) -> RuntimeResult<i32> {
    if download.is_null() { return Err(RuntimeError::new("Null canonical UDownload")); }
    let file_open = unsafe { !(*download).recv_file_ar.is_null() };
    let flags = unsafe {
        if (*download).info.is_null() { 0 } else { (*(*download).info.cast::<FPackageInfo>()).package_flags }
    };
    if skip_eligible(file_open, flags) {
        unsafe { (*download).skipped_file = 1; }
        return Ok(1);
    }
    let _ = access;
    Ok(0)
}

/// Completes cache placement and sends the canonical package notification.
///
/// # Safety
/// `download` is live; the connection and package metadata remain live through notification.
pub unsafe fn download_done(access: CoreAccess<'_>, download: *mut UDownload) -> RuntimeResult<()> {
    if download.is_null() { return Err(RuntimeError::new("Null canonical UDownload")); }
    if unsafe { (*download).info.is_null() } { return Ok(()); }
    let mut ops = CoreCompletionOps { access };
    unsafe { download_done_with(&mut ops, download) }
}

struct CoreCompletionOps<'a> {
    access: CoreAccess<'a>,
}

impl DownloadCompletionOps for CoreCompletionOps<'_> {
    type Error = RuntimeError;

    unsafe fn close_receive_file(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        unsafe { close_archive(download) }
    }
    unsafe fn skipped(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        Ok(unsafe { (*download).skipped_file != 0 })
    }
    unsafe fn package_name(&mut self, download: *mut UDownload) -> RuntimeResult<String> {
        let info = unsafe { (*download).info.cast::<FPackageInfo>() };
        unsafe { package_name(self.access, info) }
    }
    unsafe fn log_skipped(&mut self, _download: *mut UDownload, name: &str) -> RuntimeResult<()> {
        emit_download_log(self.access, "Log", &format!("Skipped download of '{name}'"))
    }
    unsafe fn remove_temp(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let temp = unsafe { temp_path(self.access, download)? };
        let _ = fs::remove_file(temp);
        Ok(())
    }
    unsafe fn progress(&mut self, download: *mut UDownload, message: &str) -> RuntimeResult<()> {
        let caption = wide("Success");
        let message = wide(message);
        unsafe {
            crate::level_net_owner::notify_progress(
                self.access,
                (*download).connection.cast(),
                caption.as_ptr(),
                message.as_ptr(),
                4.0,
            )
        }
    }
    unsafe fn notify_received(&mut self, download: *mut UDownload, skipped: bool) -> RuntimeResult<()> {
        let empty = [0u32];
        unsafe {
            crate::level_net_owner::notify_received_file(
                self.access,
                (*download).connection.cast(),
                (*download).package_index,
                if skipped { empty.as_ptr() } else { (*download).error.as_ptr() },
                skipped,
            )
        }
    }
    unsafe fn transferred(&mut self, download: *mut UDownload) -> RuntimeResult<i32> {
        Ok(unsafe { (*download).transfered })
    }
    unsafe fn has_error(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        Ok(unsafe { (*download).error[0] != 0 })
    }
    unsafe fn set_error(
        &mut self,
        download: *mut UDownload,
        key: &'static str,
        name: Option<&str>,
    ) -> RuntimeResult<()> {
        if let Some(name) = name {
            assign_localized_error_with_args(
                self.access, download, key,
                &[crate::net_download::FormatArg::Text(name)],
            )
        } else {
            assign_localized_error(self.access, download, key)
        }
    }
    unsafe fn is_compressed(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        Ok(unsafe { (*download).is_compressed != 0 })
    }
    unsafe fn decompress(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        let path = unsafe { temp_path(self.access, download)? };
        Ok(decompress_file(self.access, download, &path).is_ok())
    }
    unsafe fn size_matches(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        let info = unsafe { (*download).info.cast::<FPackageInfo>() };
        let expected = unsafe { (*info).file_size };
        let path = unsafe { temp_path(self.access, download)? };
        Ok(fs::metadata(path).is_ok_and(|metadata| metadata.len() == expected as u64))
    }
    unsafe fn move_to_cache(&mut self, download: *mut UDownload) -> RuntimeResult<bool> {
        let info = unsafe { (*download).info.cast::<FPackageInfo>() };
        let temp = unsafe { temp_path(self.access, download)? };
        let extension = system_cache_ext(self.access)?;
        let destination = system_cache_dir(self.access)?
            .join(format!("{}{extension}", unsafe { guid_string(&(*info).guid) }));
        Ok(fs::rename(temp, destination).is_ok())
    }
    unsafe fn update_cache(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let info = unsafe { (*download).info.cast::<FPackageInfo>() };
        let guid = unsafe { guid_string(&(*info).guid) };
        update_cache_ini(self.access, info, &guid)
    }
    unsafe fn flush_cache(&mut self, _download: *mut UDownload) -> RuntimeResult<()> {
        flush_cache_ini(self.access)
    }
}

struct CoreCodecBridge {
    input: Vec<u8>,
    output: Vec<u8>,
    position: usize,
    reader: *mut hp2_abi::engine::generated::FBitReader,
    writer: *mut hp2_abi::engine::generated::FBitWriter,
}

unsafe extern "C" fn codec_bridge(
    context: *mut std::ffi::c_void,
    leaf: hp2_core::download_codec::Hp2CodecLeaf,
    io: *mut hp2_core::download_codec::Hp2CodecIo,
) -> i32 {
    if context.is_null() || io.is_null() {
        return 1;
    }
    let bridge = unsafe { &mut *context.cast::<CoreCodecBridge>() };
    let io = unsafe { &mut *io };
    use hp2_core::download_codec::Hp2CodecLeaf as Leaf;
    match leaf {
        Leaf::Read => {
            if io.length < 0 || bridge.position.saturating_add(io.length as usize) > bridge.input.len() {
                return 1;
            }
            if io.length > 0 {
                if io.data.is_null() { return 1; }
                unsafe {
                    ptr::copy_nonoverlapping(
                        bridge.input.as_ptr().add(bridge.position),
                        io.data.cast(),
                        io.length as usize,
                    );
                }
            }
            bridge.position += io.length as usize;
        }
        Leaf::Write => {
            if io.length < 0 || (io.length > 0 && io.data.is_null()) {
                return 1;
            }
            let start = bridge.output.len();
            let Some(end) = start.checked_add(io.length as usize) else { return 1; };
            bridge.output.resize(end, 0);
            if io.length > 0 {
                unsafe {
                    ptr::copy_nonoverlapping(
                        io.data.cast::<u8>(),
                        bridge.output.as_mut_ptr().add(start),
                        io.length as usize,
                    );
                }
            }
        }
        Leaf::Tell => io.value = bridge.position as i32,
        Leaf::TotalSize => {
            let Ok(size) = i32::try_from(bridge.input.len()) else { return 1; };
            io.value = size;
        }
        Leaf::AtEnd => io.value = i32::from(bridge.position >= bridge.input.len()),
        Leaf::Seek => {
            if io.value < 0 || io.value as usize > bridge.input.len() { return 1; }
            bridge.position = io.value as usize;
        }
        Leaf::Progress => io.value = 1,
        Leaf::Check => return 1,
        Leaf::BitWriterBegin => {
            if io.length < 0 || !bridge.writer.is_null() { return 1; }
            let writer = Box::into_raw(Box::new(unsafe { std::mem::zeroed() }));
            if unsafe { hp2_core::bits::writer_init(writer, io.length, hp2_core::array_storage::system_allocator()) }.is_err() {
                unsafe { drop(Box::from_raw(writer)); }
                return 1;
            }
            bridge.writer = writer;
            io.data = writer.cast();
        }
        Leaf::BitReaderBegin => {
            if io.length < 0 || !bridge.reader.is_null() || (io.length > 0 && io.data.is_null()) { return 1; }
            let reader = Box::into_raw(Box::new(unsafe { std::mem::zeroed() }));
            if unsafe { hp2_core::bits::reader_init(reader, io.data.cast(), io.length, hp2_core::array_storage::system_allocator()) }.is_err() {
                unsafe { drop(Box::from_raw(reader)); }
                return 1;
            }
            bridge.reader = reader;
            io.data = reader.cast();
        }
    }
    0
}

fn decode_codec_stage(
    input: Vec<u8>,
    codec: hp2_core::download_codec::Hp2CodecKind,
) -> RuntimeResult<Vec<u8>> {
    use hp2_core::download_codec::{CodecIo, Hp2CodecKind, Hp2CodecServices};
    let mut bridge = CoreCodecBridge {
        input,
        output: Vec::new(),
        position: 0,
        reader: ptr::null_mut(),
        writer: ptr::null_mut(),
    };
    let services = Hp2CodecServices {
        context: (&raw mut bridge).cast(),
        invoke: codec_bridge,
        notify_present: 0,
    };
    // SAFETY: all codec service leaves are synchronous and context remains live.
    let io = unsafe { CodecIo::new(&services) };
    let operation = unsafe {
        match codec {
            Hp2CodecKind::Bwt => hp2_core::download_codec_bwt::decode(&io),
            Hp2CodecKind::Rle => hp2_core::download_codec_byte::rle_decode(&io),
            Hp2CodecKind::Huffman => hp2_core::download_codec_huffman::decode(&io),
            Hp2CodecKind::Mtf => hp2_core::download_codec_byte::mtf_decode(&io),
        }
    };
    let mut cleanup = Ok(());
    unsafe {
        if !bridge.reader.is_null() {
            let buffer = ptr::addr_of_mut!((*bridge.reader).buffer);
            cleanup = crate::net_bunch::release_core_buffer(buffer).map_err(|_| RuntimeError::new("Unable to release codec bit reader"));
            drop(Box::from_raw(bridge.reader));
        }
        if !bridge.writer.is_null() {
            let buffer = ptr::addr_of_mut!((*bridge.writer).buffer);
            let writer_cleanup = crate::net_bunch::release_core_buffer(buffer)
                .map_err(|_| RuntimeError::new("Unable to release codec bit writer"));
            if cleanup.is_ok() { cleanup = writer_cleanup; }
            drop(Box::from_raw(bridge.writer));
        }
    }
    if operation.is_err() {
        return Err(RuntimeError::new("Compressed package decode failed"));
    }
    cleanup?;
    if operation == Ok(0) {
        return Err(RuntimeError::new("Compressed package decode was cancelled"));
    }
    Ok(bridge.output)
}

fn decompress_file(
    access: CoreAccess<'_>,
    download: *mut UDownload,
    path: &PathBuf,
) -> RuntimeResult<PathBuf> {
    let compressed = fs::read(path)
        .map_err(|error| RuntimeError::new(&format!("Unable to read compressed package: {error}")))?;
    if compressed.len() < 4 {
        return Err(RuntimeError::new("Compressed package header is truncated"));
    }
    let signature = i32::from_le_bytes(compressed[..4].try_into().map_err(|_| RuntimeError::new("Compressed package header is invalid"))?);
    if signature != 1234 && signature != 5678 {
        return Err(RuntimeError::new("Compressed package signature is invalid"));
    }
    let mut cursor = 4usize;
    let string_bytes = hp2_core::compact_index::decode(|| {
        let Some(byte) = compressed.get(cursor).copied() else { return Err(()); };
        cursor += 1;
        Ok(byte)
    }).map_err(|_| RuntimeError::new("Compressed package header is invalid"))?;
    let string_size = string_bytes.checked_abs().ok_or_else(|| RuntimeError::new("Compressed package header is invalid"))? as usize;
    let string_size = string_size.checked_mul(if string_bytes < 0 { 2 } else { 1 })
        .ok_or_else(|| RuntimeError::new("Compressed package header is invalid"))?;
    cursor = cursor.checked_add(string_size).ok_or_else(|| RuntimeError::new("Compressed package header is invalid"))?;
    if cursor > compressed.len() {
        return Err(RuntimeError::new("Compressed package header is truncated"));
    }
    let mut decoded = compressed[cursor..].to_vec();
    let mut codecs = vec![hp2_core::download_codec::Hp2CodecKind::Huffman];
    if signature == 5678 {
        codecs.push(hp2_core::download_codec::Hp2CodecKind::Rle);
    }
    codecs.push(hp2_core::download_codec::Hp2CodecKind::Mtf);
    codecs.extend([
        hp2_core::download_codec::Hp2CodecKind::Bwt,
        hp2_core::download_codec::Hp2CodecKind::Rle,
    ]);
    for codec in codecs {
        decoded = decode_codec_stage(decoded, codec)?;
    }
    static NEXT_DECODED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT_DECODED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let output = path.with_file_name(format!("decoded-{serial}.tmp"));
    fs::write(&output, decoded)
        .map_err(|error| RuntimeError::new(&format!("Unable to write decoded package: {error}")))?;
    let filename = output.to_string_lossy().chars().map(u32::from).chain([0]).collect::<Vec<_>>();
    if filename.len() > unsafe { (*download).temp_filename.len() } {
        let _ = fs::remove_file(&output);
        return Err(RuntimeError::new("Decoded package temporary filename is too long"));
    }
    unsafe {
        (*download).temp_filename.fill(0);
        (&mut (*download).temp_filename)[..filename.len()].copy_from_slice(&filename);
    }
    let _ = fs::remove_file(path);
    Ok(output)
}

pub(crate) trait DownloadDestroyOps {
    type Error;
    unsafe fn close_and_remove_partial(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
    unsafe fn clear_connection_download(&mut self, download: *mut UDownload) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn destroy_with<O: DownloadDestroyOps>(
    ops: &mut O,
    download: *mut UDownload,
) -> Result<(), O::Error> {
    unsafe {
        ops.close_and_remove_partial(download)?;
        ops.clear_connection_download(download)
    }
}

struct CoreDestroyOps<'a> {
    access: CoreAccess<'a>,
}

impl DownloadDestroyOps for CoreDestroyOps<'_> {
    type Error = RuntimeError;
    unsafe fn close_and_remove_partial(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let had_file = unsafe { !(*download).recv_file_ar.is_null() };
        unsafe { close_archive(download)?; }
        if had_file {
            let path = unsafe { temp_path(self.access, download)? };
            let _ = fs::remove_file(path);
        }
        Ok(())
    }
    unsafe fn clear_connection_download(&mut self, download: *mut UDownload) -> RuntimeResult<()> {
        let connection = unsafe { (*download).connection.cast::<UNetConnection>() };
        if !connection.is_null() {
            unsafe {
                if (*connection).download == download.cast() {
                    (*connection).download = ptr::null_mut();
                }
                (*download).connection = ptr::null_mut();
            }
        }
        Ok(())
    }
}

pub unsafe fn destroy(access: CoreAccess<'_>, download: *mut UDownload) -> RuntimeResult<()> {
    if download.is_null() { return Err(RuntimeError::new("Null canonical UDownload")); }
    unsafe { destroy_with(&mut CoreDestroyOps { access }, download) }
}

/// Releases the channel-download object through Core object ownership.
///
/// # Safety
/// `download` is a live canonical object.
pub unsafe fn delete_channel_download(access: CoreAccess<'_>, download: *mut UChannelDownload) -> RuntimeResult<()> {
    if download.is_null() { return Ok(()); }
    unsafe { access.delete_object(download.cast()) }
}

/// Releases a channel file-send archive allocated by the canonical owner.
///
/// # Safety
/// `archive` is a `Box<File>` returned by this module.
pub unsafe fn delete_send_archive(_access: CoreAccess<'_>, archive: *mut std::ffi::c_void) -> RuntimeResult<()> {
    if !archive.is_null() { unsafe { drop(Box::from_raw(archive.cast::<File>())); } }
    Ok(())
}

unsafe fn destroy_download_object(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { destroy(access, object.cast())?; }
    (OBJECT.destroy)(access, object)
}
unsafe fn destroy_channel_download_object(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let download = object.cast::<UChannelDownload>();
    unsafe {
        let channel = (*download).ch.cast::<hp2_abi::engine::generated::UFileChannel>();
        if !channel.is_null() && (*channel).download == download.cast() {
            (*channel).download = ptr::null_mut();
        }
        (*download).ch = ptr::null_mut();
        destroy(access, download.cast::<UDownload>())?;
    }
    (OBJECT.destroy)(access, object)
}
unsafe fn close_archive(download: *mut UDownload) -> RuntimeResult<()> {
    let archive = unsafe { (*download).recv_file_ar.cast::<File>() };
    if !archive.is_null() { unsafe { drop(Box::from_raw(archive)); (*download).recv_file_ar = ptr::null_mut(); } }
    Ok(())
}
unsafe fn temp_path(access: CoreAccess<'_>, download: *mut UDownload) -> RuntimeResult<PathBuf> {
    let stored = unsafe { &(*download).temp_filename };
    if stored[0] != 0 {
        let mut filename = String::new();
        for unit in stored.iter().copied().take_while(|unit| *unit != 0) {
            let character = char::from_u32(unit)
                .ok_or_else(|| RuntimeError::new("Download temporary filename is invalid"))?;
            filename.push(character);
        }
        return Ok(PathBuf::from(filename));
    }
    let info = unsafe { (*download).info.cast::<FPackageInfo>() };
    if info.is_null() { return Err(RuntimeError::new("Download has no package metadata")); }
    static NEXT_TEMPORARY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT_TEMPORARY.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let guid = unsafe { guid_string(&(*info).guid) };
    Ok(system_cache_dir(access)?.join(format!("{guid}-{serial}.tmp")))
}

fn system_text(array: hp2_abi::engine::generated::TArrayRaw) -> RuntimeResult<String> {
    if array.num < 0 || (array.num > 0 && array.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical System path string"));
    }
    let units = if array.num == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(array.data.cast::<u32>(), array.num as usize) }
    };
    units.iter().copied().take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid System path character")))
        .collect()
}

fn system_cache_dir(access: CoreAccess<'_>) -> RuntimeResult<PathBuf> {
    let system = access.system()?;
    let configured = system_text(unsafe { (*system).cache_path })?;
    let path = PathBuf::from(configured);
    Ok(if path.is_absolute() { path } else { access.data_root().join("System").join(path) })
}

fn system_cache_ext(access: CoreAccess<'_>) -> RuntimeResult<String> {
    let system = access.system()?;
    system_text(unsafe { (*system).cache_ext })
}
unsafe fn guid_string(guid: &[u32; 4]) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:04X}-{:04X}{:08X}",
        guid[0],
        guid[1] >> 16,
        guid[1] & 0xffff,
        guid[2] >> 16,
        guid[2] & 0xffff,
        guid[3],
    )
}

fn update_cache_ini(access: CoreAccess<'_>, info: *mut FPackageInfo, guid: &str) -> RuntimeResult<()> {
    let cache_filename = system_cache_dir(access)?.join("cache.ini");
    let cache_filename = cache_filename.to_string_lossy().chars().map(u32::from).collect::<Vec<_>>();
    let section = "Cache".chars().map(u32::from).collect::<Vec<_>>();
    let key = guid.chars().map(u32::from).collect::<Vec<_>>();
    let url = unsafe { (*info).url };
    let value = if !url.data.is_null() && url.num > 0 {
        unsafe { std::slice::from_raw_parts(url.data.cast::<u32>(), url.num as usize) }
            .iter().copied().take_while(|unit| *unit != 0).collect::<Vec<_>>()
    } else {
        let parent = unsafe { (*info).parent.cast::<UObject>() };
        if parent.is_null() { return Err(RuntimeError::new("Downloaded package has no parent object")); }
        let name = i32::from_ne_bytes(unsafe { (*parent).name });
        access.name_text(name)?
    };
    access.config_set_units_from(Some(&cache_filename), &section, &key, &value)
}

fn flush_cache_ini(access: CoreAccess<'_>) -> RuntimeResult<()> {
    let cache_filename = system_cache_dir(access)?.join("cache.ini");
    let cache_filename = cache_filename.to_string_lossy().chars().map(u32::from).collect::<Vec<_>>();
    access.config_flush_units_from(Some(&cache_filename), false)
}

/// Executes the inherited download tick.
///
/// `UnDownload.h:38` defines UDownload::Tick as an empty virtual method;
/// UChannelDownload does not override it. Transfer progress is packet-driven
/// through ReceiveData, not a synthetic polling operation here.
///
/// # Safety
/// `download` is a live canonical Download or ChannelDownload record.
pub unsafe fn tick(_access: CoreAccess<'_>, _download: *mut UDownload) -> RuntimeResult<()> {
    Ok(())
}
