//! Canonical `NetPendingLevel` and `DemoPendingLevel` text-notification ownership.

use hp2_abi::engine::generated::{
    APlayerPawn, APlayerReplicationInfo, FDownloadInfo, FPackageInfo, FURL, TArrayRaw, UClient,
    UDemoPlayPendingLevel, UDemoRecDriver, UNetConnection, UNetDriver, UNetPendingLevel,
    UPackageMap, UEngine, ULevelBase, UObject, UViewport,
};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_core::text_parse;
use std::{ffi::{c_void, CStr}, ptr};

struct CoreNetTextOps<'a> {
    access: CoreAccess<'a>,
    retained: Vec<Vec<u32>>,
}

impl CoreNetTextOps<'_> {
    fn retain(&mut self, mut value: Vec<u32>) -> *const u32 {
        if value.last().copied() != Some(0) { value.push(0); }
        self.retained.push(value);
        self.retained.last().expect("retained Core text was just appended").as_ptr()
    }
}

impl crate::net_pending::NotifyReceivedTextOps for CoreNetTextOps<'_> {
    type Error = RuntimeError;

    fn check_connection(&mut self, this: *mut c_void, conn: *mut c_void) -> RuntimeResult<()> {
        let pending = this.cast::<UNetPendingLevel>();
        if pending.is_null() || conn.is_null() {
            return Err(RuntimeError::new("Invalid canonical pending notification arguments"));
        }
        let driver = unsafe { ptr::read_unaligned(ptr::addr_of!((*pending).base.net_driver)) }.cast::<UNetDriver>();
        let server = if driver.is_null() { ptr::null_mut() } else {
            unsafe { ptr::read_unaligned(ptr::addr_of!((*driver).server_connection)) }
        };
        if server != conn { return Err(RuntimeError::new("Canonical pending connection mismatch")); }
        Ok(())
    }
    fn log_received_text(&mut self, text: *const u32) -> RuntimeResult<()> {
        let mut message = wide("PendingLevel received: ");
        message.extend(unsafe { nul_terminated(text)? });
        let category = self.access.intern_str("DevNet", FindName::Add)?;
        self.access.emit_log(category, &message)
    }
    fn log_welcome(&mut self, text: *const u32) -> RuntimeResult<()> {
        let mut message = wide("Welcomed by server: ");
        message.extend(unsafe { nul_terminated(text)? });
        let category = self.access.intern_str("DevNet", FindName::Add)?;
        self.access.emit_log(category, &message)
    }
    fn parse_command(&mut self, text: &mut *const u32, command: &[u32]) -> RuntimeResult<bool> {
        Ok(unsafe { hp2_core::command::parse_command(ptr::from_mut(text).cast(), command.as_ptr().cast()) != 0 })
    }
    fn parse_int(&mut self, text: *const u32, key: &[u32], value: &mut i32) -> RuntimeResult<()> {
        let mut raw = [0; 256];
        if unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), raw.as_mut_ptr().cast(), raw.len() as i32) } {
            *value = c_atoi(&raw) as i32;
        }
        Ok(())
    }
    fn parse_dword(&mut self, text: *const u32, key: &[u32], value: &mut u32) -> RuntimeResult<()> {
        let mut raw = *value as i32;
        self.parse_int(text, key, &mut raw)?;
        *value = raw as u32;
        Ok(())
    }
    fn parse_guid(&mut self, text: *const u32, key: &[u32], output: &mut [u32; 4]) -> RuntimeResult<()> {
        let mut raw = [0; 256];
        if !unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), raw.as_mut_ptr().cast(), raw.len() as i32) } { return Ok(()); }
        parse_guid_fields(&raw, output);
        Ok(())
    }
    fn parse_text(&mut self, text: *const u32, key: &[u32], output: &mut [u32; hp2_core::name::NAME_SIZE]) -> RuntimeResult<()> {
        unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), output.as_mut_ptr().cast(), output.len() as i32); }
        Ok(())
    }
    fn parse_fstring(&mut self, text: *const u32, key: &[u32], destination: *mut TArrayRaw) -> RuntimeResult<()> {
        if let Some(raw) = parse_fstring_buffer(text, key)? {
            unsafe { self.access.assign_fstring(destination, &raw)?; }
        }
        Ok(())
    }
    fn parse_ubool(&mut self, text: *const u32, key: &[u32], output: &mut i32) -> RuntimeResult<()> {
        let mut localized_true = self.access.localize_text(
            hp2_core::lit!("Core"), hp2_core::lit!("True"), Some(hp2_core::lit!("Core")), None, false,
        )?;
        if localized_true.last().copied() != Some(0) { localized_true.push(0); }
        let _ = unsafe {
            hp2_core::parse_scalar::parse_bool(text.cast(), key.as_ptr().cast(), localized_true.as_ptr().cast(), output)
        };
        Ok(())
    }
    fn parse_atoi(&mut self, text: *const u32) -> RuntimeResult<i32> {
        Ok(c_atoi(&unsafe { nul_terminated(text)? }) as i32)
    }
    fn localize_error(&mut self, key: &CStr) -> RuntimeResult<*const u32> {
        let key: Vec<u32> = key.to_bytes().iter().map(|byte| u32::from(*byte)).chain([0]).collect();
        let value = self.access.localize_text(hp2_core::lit!("Errors"), &key, Some(hp2_core::lit!("Engine")), None, false)?;
        Ok(self.retain(value))
    }
    fn assign_fstring(&mut self, destination: *mut TArrayRaw, text: *const u32) -> RuntimeResult<()> {
        let mut units = unsafe { nul_terminated(text)? };
        units.push(0);
        unsafe { self.access.assign_fstring(destination, &units) }
    }
    fn pending_url(&mut self, this: *mut c_void) -> RuntimeResult<*mut c_void> {
        let pending = this.cast::<UNetPendingLevel>();
        let url = unsafe { ptr::addr_of_mut!((*pending).base.url).cast::<FURL>() };
        Ok(url.cast())
    }
    fn url_op_count(&mut self, url: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*url.cast::<FURL>()).op)).num } }
    fn url_op_has_prefix(&mut self, url: *mut c_void, index: i32, prefix: &[u32]) -> bool {
        unsafe { op_has_prefix(url, index, prefix) }
    }
    fn remove_url_op(&mut self, url: *mut c_void, index: i32) -> RuntimeResult<()> {
        unsafe { remove_url_option(self.access, url.cast(), index) }
    }
    fn url_string_assign(&mut self, url: *mut c_void, destination: *mut TArrayRaw) -> RuntimeResult<()> {
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
        let mut value = unsafe { crate::url_owner::UrlOwner::string(self.access, &defaults, url.cast(), false)? };
        let result = unsafe { fstring_units(value).and_then(|mut units| { units.push(0); self.access.assign_fstring(destination, &units) }) };
        let destroyed = unsafe { self.access.destroy_owned_fstring(&mut value) };
        result.and(destroyed)
    }
    fn with_url_copy<F>(&mut self, url: *mut c_void, body: F) -> RuntimeResult<()>
    where F: FnOnce(&mut Self, *mut c_void) -> RuntimeResult<()> {
        let mut copy = std::mem::MaybeUninit::<FURL>::uninit();
        unsafe { crate::url_owner::UrlOwner::copy_construct(self.access, copy.as_mut_ptr(), url.cast())?; }
        let copy = unsafe { copy.assume_init() };
        let result = body(self, ptr::from_ref(&copy).cast_mut().cast());
        let mut copy = copy;
        let destroyed = unsafe { crate::url_owner::UrlOwner::destroy(self.access, ptr::from_mut(&mut copy)) };
        result.and(destroyed)
    }
    fn with_url_string<F>(&mut self, url: *mut c_void, body: F) -> RuntimeResult<()>
    where F: FnOnce(&mut Self, *const u32) -> RuntimeResult<()> {
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
        let value = unsafe { crate::url_owner::UrlOwner::string(self.access, &defaults, url.cast(), false)? };
        if value.data.is_null() { return body(self, [0].as_ptr()); }
        let result = body(self, value.data.cast());
        let mut value = value;
        let destroyed = unsafe { self.access.destroy_owned_fstring(&mut value) };
        result.and(destroyed)
    }
    fn add_url_option(&mut self, url: *mut c_void, text: *const u32) -> RuntimeResult<()> {
        let defaults = crate::url_owner::UrlOwner::current_defaults(self.access)?;
        unsafe { crate::url_owner::UrlOwner::add_option(self.access, &defaults, url.cast(), text) }
    }
    fn create_package(&mut self, name: &[u32]) -> RuntimeResult<*mut c_void> { create_package(self.access, name) }
    fn append_package_info(&mut self, map: *mut c_void) -> RuntimeResult<*mut FPackageInfo> {
        if map.is_null() { return Err(RuntimeError::new("Null canonical package map")); }
        let list = unsafe { ptr::addr_of_mut!((*map.cast::<UPackageMap>()).list) };
        let slots = Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*list).data) }, num: unsafe { ptr::addr_of_mut!((*list).num) }, max: unsafe { ptr::addr_of_mut!((*list).max) } };
        let index = unsafe { array_storage::add_zeroed(slots,1,std::mem::size_of::<FPackageInfo>() as i32,ptr::null(),array_storage::system_allocator()) }.map_err(|_| RuntimeError::new("Unable to append canonical package info"))?;
        Ok(unsafe { (*list).data.cast::<FPackageInfo>().add(index as usize) })
    }
    fn stats_checksum(&mut self, this: *mut c_void) -> RuntimeResult<Vec<u32>> {
        let level = this.cast::<ULevelBase>();
        let engine = unsafe { ptr::read_unaligned(ptr::addr_of!((*level).engine)) }.cast::<UEngine>();
        if engine.is_null() { return Err(RuntimeError::new("Canonical pending level has no engine")); }
        let client = unsafe { ptr::read_unaligned(ptr::addr_of!((*engine).client)) }.cast::<UClient>();
        if client.is_null() { return Err(RuntimeError::new("Canonical engine has no client")); }
        let views = unsafe { ptr::read_unaligned(ptr::addr_of!((*client).viewports)) };
        if views.num < 0 || (views.num > 0 && views.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical viewport storage"));
        }
        if views.num == 0 { return Err(RuntimeError::new("Canonical client has no viewport")); }
        let viewport = unsafe { ptr::read_unaligned(views.data.cast::<*mut c_void>()) }.cast::<UViewport>();
        if viewport.is_null() { return Err(RuntimeError::new("Canonical client has a null first viewport")); }
        let pawn = unsafe { ptr::read_unaligned(ptr::addr_of!((*viewport).actor)) }.cast::<APlayerPawn>();
        if pawn.is_null() { return Ok(wide("Checksum=NoChecksum")); }
        let secret = unsafe { fstring_units(ptr::read_unaligned(ptr::addr_of!((*pawn).ng_world_secret)))? };
        if secret.is_empty() { return Ok(wide("Checksum=NoChecksum")); }
        let pri = unsafe { ptr::read_unaligned(ptr::addr_of!((*pawn).base.player_replication_info)) }.cast::<APlayerReplicationInfo>();
        let name = if pri.is_null() { Vec::new() } else {
            unsafe { fstring_units(ptr::read_unaligned(ptr::addr_of!((*pri).player_name)))? }
        };
        let name_bytes = crate::stat_log::encode_utf16le(hp2_core::native_text::tchar_units(&name), false)
            .ok_or_else(|| RuntimeError::new("Canonical player name is too long to checksum"))?;
        let secret_bytes = crate::stat_log::encode_utf16le(hp2_core::native_text::tchar_units(&secret), false)
            .ok_or_else(|| RuntimeError::new("Canonical world secret is too long to checksum"))?;
        let mut context = std::mem::MaybeUninit::<hp2_core::md5::Md5Context>::uninit();
        unsafe {
            hp2_core::md5::init(context.as_mut_ptr());
            hp2_core::md5::update(context.as_mut_ptr(), name_bytes.as_ptr(), name_bytes.len() as i32);
            hp2_core::md5::update(context.as_mut_ptr(), secret_bytes.as_ptr(), secret_bytes.len() as i32);
        }
        let mut digest = [0u8; 16];
        unsafe { hp2_core::md5::final_digest(digest.as_mut_ptr(), context.as_mut_ptr()); }
        let mut option = wide("Checksum=");
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in digest {
            option.push(u32::from(HEX[usize::from(byte >> 4)]));
            option.push(u32::from(HEX[usize::from(byte & 0x0f)]));
        }
        option.push(0);
        Ok(option)
    }
    fn log_connection(&mut self, conn: *mut c_void, text: *const u32) -> RuntimeResult<()> {
        if conn.is_null() { return Err(RuntimeError::new("Null canonical connection")); }
        let channel = unsafe { ptr::read_unaligned(ptr::addr_of!((*conn.cast::<UNetConnection>()).channels[0])) }
            .cast::<hp2_abi::engine::generated::UControlChannel>();
        if channel.is_null() { return Err(RuntimeError::new("Null canonical control channel")); }
        unsafe { crate::channel_owner::serialize_control(self.access, channel, text) }
    }
    fn challenge_response(&mut self, _this: *mut c_void, challenge: i32) -> RuntimeResult<i32> { Ok(crate::game_engine::challenge_response(challenge)) }
    fn flush_connection(&mut self, conn: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::connection_owner::flush_net(self.access, conn.cast()) }
    }
    fn add_download_info(&mut self, conn: *mut c_void) -> RuntimeResult<(i32, *mut FDownloadInfo)> {
        let conn = conn.cast::<UNetConnection>();
        let list = unsafe { ptr::addr_of_mut!((*conn).download_info) };
        let slots = Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*list).data) }, num: unsafe { ptr::addr_of_mut!((*list).num) }, max: unsafe { ptr::addr_of_mut!((*list).max) } };
        let index = unsafe { array_storage::add_zeroed(slots,1,std::mem::size_of::<FDownloadInfo>() as i32,ptr::null(),array_storage::system_allocator()) }.map_err(|_| RuntimeError::new("Unable to append canonical download info"))?;
        Ok((index, unsafe { (*list).data.cast::<FDownloadInfo>().add(index as usize) }))
    }
    fn load_download_class(&mut self, name: *const u32) -> RuntimeResult<*mut c_void> {
        let name = unsafe { nul_terminated(name)? };
        let base = self.access.native_class("Engine.Download")?;
        let class = unsafe { self.access.static_load_class(base, ptr::null_mut(), &name, 0x2002)? };
        Ok(class.cast())
    }
    fn remove_download_info(&mut self, conn: *mut c_void, index: i32) -> RuntimeResult<()> {
        let list = unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).download_info) };
        let raw = unsafe { ptr::read_unaligned(list) };
        if index < 0 || index >= raw.num || raw.data.is_null() { return Err(RuntimeError::new("Canonical download-info index out of range")); }
        let info = unsafe { raw.data.cast::<FDownloadInfo>().add(index as usize) };
        unsafe { self.access.destroy_owned_fstring(ptr::addr_of_mut!((*info).class_name))?; self.access.destroy_owned_fstring(ptr::addr_of_mut!((*info).params))?; }
        let slots = Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*list).data) }, num: unsafe { ptr::addr_of_mut!((*list).num) }, max: unsafe { ptr::addr_of_mut!((*list).max) } };
        unsafe { array_storage::remove(slots,index,1,std::mem::size_of::<FDownloadInfo>() as i32,ptr::null(),array_storage::system_allocator()) }.map_err(|_| RuntimeError::new("Unable to remove canonical download info"))
    }
    fn package_name(&mut self, info: *mut FPackageInfo) -> RuntimeResult<*const u32> {
        let parent = unsafe { ptr::read_unaligned(ptr::addr_of!((*info).parent)) };
        if parent.is_null() { return Err(RuntimeError::new("Canonical package has no parent")); }
        let index = i32::from_ne_bytes(unsafe { ptr::read_unaligned(ptr::addr_of!((*parent.cast::<UObject>()).name)) });
        let name = self.access.name_text(index)?;
        Ok(self.retain(name))
    }
    fn find_package_file(&mut self, name: *const u32, guid: *const c_void, filename: &mut [u32; 256]) -> RuntimeResult<bool> {
        let name = unsafe { nul_terminated(name)? };
        let guid = unsafe { ptr::read_unaligned(guid.cast::<[u32; 4]>()) };
        hp2_core::package_file_owner::find_package_file(self.access, &name, Some(&guid), filename)
    }
    fn dll_extension(&mut self) -> RuntimeResult<*const u32> {
        Ok(hp2_core::package_file_owner::dll_extension().as_ptr())
    }
    fn file_size(&mut self, filename: *const u32) -> RuntimeResult<i32> {
        let filename = unsafe { nul_terminated(filename)? };
        let path = hp2_core::native_path::path_from_units(&filename)?;
        hp2_core::unix_file_manager_owner::file_size(
            self.access, &hp2_core::package_file_owner::system_path(self.access, &path),
        )
    }
    fn allow_downloads(&mut self, this: *mut c_void) -> bool {
        let driver = unsafe { ptr::read_unaligned(ptr::addr_of!((*this.cast::<UNetPendingLevel>()).base.net_driver)) }.cast::<UNetDriver>();
        !driver.is_null() && unsafe { ptr::read_unaligned(ptr::addr_of!((*driver).allow_downloads)) != 0 }
    }
    fn package_map(&mut self, conn: *mut c_void) -> *mut c_void { unsafe { ptr::read_unaligned(ptr::addr_of!((*conn.cast::<UNetConnection>()).package_map)) } }
    fn package_count(&mut self, map: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*map.cast::<UPackageMap>()).list)).num } }
    fn package_at(&mut self, map: *mut c_void, index: i32) -> *mut FPackageInfo {
        unsafe { (*map.cast::<UPackageMap>()).list.data.cast::<FPackageInfo>().add(index as usize) }
    }
    fn close_first_channel(&mut self, conn: *mut c_void) -> RuntimeResult<()> {
        let channel = unsafe { ptr::read_unaligned(ptr::addr_of!((*conn.cast::<UNetConnection>()).channels[0])) }.cast::<hp2_abi::engine::generated::UChannel>();
        if channel.is_null() { return Err(RuntimeError::new("Null canonical first channel")); }
        unsafe { crate::channel_owner::close(self.access, channel) }
    }
    fn set_success(&mut self, this: *mut c_void) { unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.success).write_unaligned(1); } }
    fn package_flags(&mut self, info: *mut FPackageInfo) -> u32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*info).package_flags)) } }
    fn request_file(&mut self, conn: *mut c_void, index: i32) -> RuntimeResult<()> { unsafe { crate::connection_owner::receive_file(self.access, conn.cast(), index) } }
    fn delete_finished_download(&mut self, conn: *mut c_void) -> RuntimeResult<()> {
        let download = unsafe { ptr::read_unaligned(ptr::addr_of!((*conn.cast::<UNetConnection>()).download)) };
        if download.is_null() { Ok(()) } else { unsafe { crate::download_owner::destroy(self.access, download.cast()) } }
    }
    fn error_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.error) } }
    fn fail_code_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.fail_code) } }
    fn fail_url_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.fail_url) } }
    fn user_flags_slot(&mut self, conn: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).user_flags) } }
    fn negotiated_version_slot(&mut self, conn: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).negotiated_ver) } }
    fn challenge_slot(&mut self, conn: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).challenge) } }
    fn current_speed(&mut self, conn: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*conn.cast::<UNetConnection>()).current_net_speed)) } }
    fn remote_version(&mut self, conn: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*conn.cast::<UNetConnection>()).remote_version)) } }
    fn close_old_connection(&mut self, conn: *mut c_void) { unsafe { ptr::addr_of_mut!((*conn.cast::<UNetConnection>()).state).write_unaligned(crate::net_demo::USOCK_CLOSED); } }
    fn package_guid_slot(&mut self, info: *mut FPackageInfo) -> *mut [u32; 4] { unsafe { ptr::addr_of_mut!((*info).guid) } }
    fn package_generation_slot(&mut self, info: *mut FPackageInfo) -> *mut i32 { unsafe { ptr::addr_of_mut!((*info).remote_generation) } }
    fn package_size_slot(&mut self, info: *mut FPackageInfo) -> *mut i32 { unsafe { ptr::addr_of_mut!((*info).file_size) } }
    fn package_filename_slot(&mut self, info: *mut FPackageInfo) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*info).url) } }
    fn package_flags_slot(&mut self, info: *mut FPackageInfo) -> *mut u32 { unsafe { ptr::addr_of_mut!((*info).package_flags) } }
    fn set_package_parent(&mut self, info: *mut FPackageInfo, parent: *mut c_void) { unsafe { ptr::addr_of_mut!((*info).parent).write_unaligned(parent); } }
    fn set_package_flags(&mut self, info: *mut FPackageInfo, flags: u32) { unsafe { ptr::addr_of_mut!((*info).package_flags).write_unaligned(flags); } }
    fn increment_files_needed(&mut self, this: *mut c_void) {
        unsafe { let slot = ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.files_needed); slot.write_unaligned(slot.read_unaligned().wrapping_add(1)); }
    }
    fn url_host_slot(&mut self, url: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*url.cast::<FURL>()).host) } }
    fn server_connection(&mut self, this: *mut c_void) -> *mut c_void {
        let driver = unsafe { ptr::read_unaligned(ptr::addr_of!((*this.cast::<UNetPendingLevel>()).base.net_driver)) }.cast::<UNetDriver>();
        if driver.is_null() { ptr::null_mut() } else { unsafe { ptr::read_unaligned(ptr::addr_of!((*driver).server_connection)) } }
    }
    fn url_map_slot(&mut self, url: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*url.cast::<FURL>()).map) } }
    fn download_class_name_slot(&mut self, info: *mut FDownloadInfo) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*info).class_name) } }
    fn download_params_slot(&mut self, info: *mut FDownloadInfo) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*info).params) } }
    fn download_compression_slot(&mut self, info: *mut FDownloadInfo) -> *mut i32 { unsafe { ptr::addr_of_mut!((*info).compression) } }
    fn lone_player_slot(&mut self, this: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.lone_player) } }
    fn download_class_name(&mut self, info: *mut FDownloadInfo) -> *const u32 {
        static EMPTY: [u32; 1] = [0];
        unsafe {
            let value = ptr::read_unaligned(ptr::addr_of!((*info).class_name));
            if value.data.is_null() && value.num == 0 { EMPTY.as_ptr() } else { value.data.cast() }
        }
    }
    fn set_download_class(&mut self, info: *mut FDownloadInfo, class: *mut c_void) { unsafe { ptr::addr_of_mut!((*info).class).write_unaligned(class); } }
}

/// Runs the shared NetPending text policy directly on canonical Core records.
///
/// # Safety
/// `pending`, `connection`, and the NUL-terminated text must be live canonical records.
pub unsafe fn net_received_text(
    access: CoreAccess<'_>,
    pending: *mut UNetPendingLevel,
    connection: *mut UNetConnection,
    text: *const u32,
) -> RuntimeResult<()> {
    if pending.is_null() || connection.is_null() || text.is_null() {
        return Err(RuntimeError::new("Invalid canonical pending notification arguments"));
    }
    let mut ops = CoreNetTextOps { access, retained: Vec::new() };
    unsafe { crate::net_pending::notify_received_text_with(&mut ops, pending.cast(), connection.cast(), text) }
}

/// Sets the demo driver's third-person client flag without invoking a native vtable.
///
/// # Safety
/// `driver` is a live canonical UDemoRecDriver.
pub(crate) unsafe fn set_demo_client_third_person(
    driver: *mut UDemoRecDriver,
) -> RuntimeResult<()> {
    if driver.is_null() {
        return Err(RuntimeError::new("Null canonical demo driver"));
    }
    unsafe { ptr::addr_of_mut!((*driver).client_third_person).write_unaligned(1); }
    Ok(())
}

/// Clears only the four-byte `UNetDriver::Time` field written by the native code.
///
/// # Safety
/// `driver` is a live canonical UNetDriver.
pub(crate) unsafe fn clear_driver_time(driver: *mut UNetDriver) -> RuntimeResult<()> {
    if driver.is_null() {
        return Err(RuntimeError::new("Null canonical network driver"));
    }
    unsafe {
        ptr::addr_of_mut!((*driver).time)
            .cast::<f32>()
            .write_unaligned(0.0);
    }
    Ok(())
}

/// Tests a URL FString option against an ASCII-case-insensitive prefix.
///
/// # Safety
/// `url` must be a live FURL whose option array contains initialized FStrings.
unsafe fn op_has_prefix(url: *mut c_void, index: i32, prefix: &[u32]) -> bool {
    if url.is_null() { return false; }
    let options = unsafe { ptr::read_unaligned(ptr::addr_of!((*url.cast::<FURL>()).op)) };
    if index < 0 || index >= options.num || options.data.is_null() { return false; }
    let value = unsafe { ptr::read_unaligned(options.data.cast::<TArrayRaw>().add(index as usize)) };
    let prefix_len = prefix.iter().position(|unit| *unit == 0).unwrap_or(prefix.len());
    if prefix_len == 0 { return true; }
    if value.num < 0 || value.num < prefix_len as i32 || value.data.is_null() { return false; }
    let units = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), prefix_len) };
    units.iter().zip(&prefix[..prefix_len]).all(|(left, right)| {
        let fold = |unit: u32| if (u32::from(b'A')..=u32::from(b'Z')).contains(&unit) { unit + 32 } else { unit };
        fold(*left) == fold(*right)
    })
}

/// Removes one canonical URL option and destroys its owned FString.
///
/// # Safety
/// `url` is a live canonical FURL and `index` is within its initialized option array.
pub(crate) unsafe fn remove_url_option(
    access: CoreAccess<'_>,
    url: *mut FURL,
    index: i32,
) -> RuntimeResult<()> {
    if url.is_null() {
        return Err(RuntimeError::new("Null canonical URL"));
    }
    let options = unsafe { ptr::addr_of_mut!((*url).op) };
    let raw = unsafe { ptr::read_unaligned(options) };
    if index < 0 || index >= raw.num || raw.data.is_null() {
        return Err(RuntimeError::new("Canonical URL option index is out of range"));
    }
    let item = unsafe { raw.data.cast::<TArrayRaw>().add(index as usize) };
    unsafe { access.destroy_owned_fstring(item)?; }
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*options).data) },
        num: unsafe { ptr::addr_of_mut!((*options).num) },
        max: unsafe { ptr::addr_of_mut!((*options).max) },
    };
    unsafe {
        array_storage::remove(
            slots,
            index,
            1,
            std::mem::size_of::<TArrayRaw>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .map_err(|_| RuntimeError::new("Unable to remove canonical URL option"))
}

/// Finds or creates the package object named by a remote `USES` record.
pub(crate) fn create_package(access: CoreAccess<'_>, name: &[u32]) -> RuntimeResult<*mut c_void> {
    use hp2_core::name_owner::FindName;

    // `UObject::CreatePackage(NULL, Name)`: resolve, find the package, then
    // construct it at the root outer. This mirrors the Core console port of the
    // same source operation instead of reaching private reflection lookups.
    let name: Vec<u32> = name.iter().copied().take_while(|unit| *unit != 0).collect();
    let package_class = access.native_class("Core.Package")?;
    if let Some(package) = unsafe { access.find_object_by_name(package_class, &name)? } {
        return Ok(package.cast());
    }
    let name_index = access.intern(&name, FindName::Add)?;
    let package = unsafe {
        access.construct_object(package_class, ptr::null_mut(), name_index, 0, ptr::null_mut())?
    };
    if package.is_null() {
        return Err(RuntimeError::new("Unable to create canonical package"));
    }
    Ok(package.cast())
}

/// Adds a wire-provided package record to a canonical package map.
///
/// # Safety
/// `map` is a live writable Core package map and `parent` is its resolved canonical package.
pub(crate) unsafe fn append_wire_package(
    access: CoreAccess<'_>,
    map: *mut UPackageMap,
    guid: [u32; 4],
    generation: i32,
    file_size: i32,
    flags: u32,
    parent: *mut c_void,
    filename: &[u32],
) -> RuntimeResult<i32> {
    if map.is_null() || parent.is_null() {
        return Err(RuntimeError::new("Invalid canonical wire package record"));
    }
    let list = unsafe { ptr::addr_of_mut!((*map).list) };
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*list).data) },
        num: unsafe { ptr::addr_of_mut!((*list).num) },
        max: unsafe { ptr::addr_of_mut!((*list).max) },
    };
    let index = unsafe {
        array_storage::add_zeroed(
            slots,
            1,
            std::mem::size_of::<FPackageInfo>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .map_err(|_| RuntimeError::new("Unable to append canonical wire package record"))?;
    let info = unsafe { (*list).data.cast::<FPackageInfo>().add(index as usize) };
    unsafe {
        ptr::addr_of_mut!((*info).guid).write_unaligned(guid);
        ptr::addr_of_mut!((*info).remote_generation).write_unaligned(generation);
        ptr::addr_of_mut!((*info).file_size).write_unaligned(file_size);
        ptr::addr_of_mut!((*info).package_flags).write_unaligned(flags);
        ptr::addr_of_mut!((*info).parent).write_unaligned(parent);
    }
    if let Err(error) = unsafe { access.assign_fstring(ptr::addr_of_mut!((*info).url), filename) } {
        return match unsafe {
            array_storage::remove(
                slots,
                index,
                1,
                std::mem::size_of::<FPackageInfo>() as i32,
                ptr::null(),
                array_storage::system_allocator(),
            )
        } {
            Ok(()) => Err(error),
            Err(_) => Err(error.with_context("Failed to roll back canonical package-map append")),
        };
    }
    Ok(index)
}

/// Releases one wire package record's owned URL and removes its array slot.
///
/// # Safety
/// `map` is live and `index` identifies an initialized record created by this Core owner.
pub(crate) unsafe fn remove_wire_package(
    access: CoreAccess<'_>,
    map: *mut UPackageMap,
    index: i32,
) -> RuntimeResult<()> {
    if map.is_null() {
        return Err(RuntimeError::new("Null canonical package map"));
    }
    let list: *mut TArrayRaw = unsafe { ptr::addr_of_mut!((*map).list) };
    let raw = unsafe { ptr::read_unaligned(list) };
    if index < 0 || index >= raw.num || raw.data.is_null() {
        return Err(RuntimeError::new("Canonical package-map index is out of range"));
    }
    let info = unsafe { raw.data.cast::<FPackageInfo>().add(index as usize) };
    unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*info).url))?; }
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*list).data) },
        num: unsafe { ptr::addr_of_mut!((*list).num) },
        max: unsafe { ptr::addr_of_mut!((*list).max) },
    };
    unsafe {
        array_storage::remove(
            slots,
            index,
            1,
            std::mem::size_of::<FPackageInfo>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .map_err(|_| RuntimeError::new("Unable to remove canonical wire package record"))
}

struct CoreDemoTextOps<'a> {
    access: CoreAccess<'a>,
    pending: *mut UDemoPlayPendingLevel,
}

impl crate::demo_pending::NotifyReceivedTextOps for CoreDemoTextOps<'_> {
    type Error = RuntimeError;

    fn log_received_text(&mut self, text: *const u32) -> RuntimeResult<()> {
        let mut message = "DemoPlayPendingLevel received: ".chars().map(u32::from).collect::<Vec<_>>();
        message.extend(unsafe { nul_terminated(text)? });
        let category = self.access.intern_str("DevNet", FindName::Add)?;
        self.access.emit_log(category, &message)
    }

    fn parse_command(&mut self, text: &mut *const u32, command: &[u32]) -> RuntimeResult<bool> {
        Ok(unsafe { hp2_core::command::parse_command(ptr::from_mut(text).cast(), command.as_ptr().cast()) != 0 })
    }

    fn parse_guid(&mut self, text: *const u32, key: &[u32], output: &mut [u32; 4]) -> RuntimeResult<()> {
        let mut raw = [0u32; 256];
        if !unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), raw.as_mut_ptr().cast(), raw.len() as i32) } {
            return Ok(());
        }
        parse_guid_fields(&raw, output);
        Ok(())
    }

    fn parse_int(&mut self, text: *const u32, key: &[u32], output: &mut i32) -> RuntimeResult<()> {
        let mut raw = [0u32; 256];
        if unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), raw.as_mut_ptr().cast(), raw.len() as i32) } {
            *output = c_atoi(&raw) as i32;
        }
        Ok(())
    }

    fn parse_dword(&mut self, text: *const u32, key: &[u32], output: &mut u32) -> RuntimeResult<()> {
        let mut value = *output as i32;
        self.parse_int(text, key, &mut value)?;
        *output = value as u32;
        Ok(())
    }

    fn parse_text(
        &mut self,
        text: *const u32,
        key: &[u32],
        output: &mut [u32; hp2_core::name::NAME_SIZE],
    ) -> RuntimeResult<()> {
        unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), output.as_mut_ptr().cast(), output.len() as i32); }
        Ok(())
    }

    fn append_package_info(
        &mut self,
        conn: *mut c_void,
        guid: &[u32; 4],
        generation: i32,
        file_size: i32,
        flags: u32,
        package_name: &[u32; hp2_core::name::NAME_SIZE],
    ) -> RuntimeResult<()> {
        let connection = conn.cast::<UNetConnection>();
        if connection.is_null() {
            return Err(RuntimeError::new("Null canonical demo connection"));
        }
        let map = unsafe { ptr::read_unaligned(ptr::addr_of!((*connection).package_map)) }
            .cast::<UPackageMap>();
        let package = create_package(self.access, package_name)?;
        let empty = [0];
        unsafe {
            append_wire_package(
                self.access,
                map,
                *guid,
                generation,
                file_size,
                flags,
                package,
                &empty,
            )?;
        }
        Ok(())
    }

    fn verify_packages(&mut self, conn: *mut c_void) -> RuntimeResult<bool> {
        let connection = conn.cast::<UNetConnection>();
        if connection.is_null() {
            return Err(RuntimeError::new("Null canonical demo connection"));
        }
        let map = unsafe { ptr::read_unaligned(ptr::addr_of!((*connection).package_map)) }
            .cast::<UPackageMap>();
        if map.is_null() {
            return Err(RuntimeError::new("Null canonical demo package map"));
        }
        let list = unsafe { ptr::read_unaligned(ptr::addr_of!((*map).list)) };
        if list.num < 0 || (list.num != 0 && list.data.is_null()) {
            return Err(RuntimeError::new("Invalid canonical demo package-map storage"));
        }
        for index in 0..list.num as usize {
            let info = unsafe { list.data.cast::<FPackageInfo>().add(index) };
            let parent = unsafe { ptr::read_unaligned(ptr::addr_of!((*info).parent)) };
            if parent.is_null() {
                return Err(RuntimeError::new("Demo package has no canonical parent"));
            }
            let name = self.access.name_text(i32::from_ne_bytes(unsafe {
                ptr::read_unaligned(ptr::addr_of!((*parent.cast::<UObject>()).name))
            }))?;
            let guid = unsafe { ptr::read_unaligned(ptr::addr_of!((*info).guid)) };
            if !package_file_exists(self.access, &name, &guid)? {
                let mut message = "Don't have package for demo: ".chars().map(u32::from).collect::<Vec<_>>();
                message.extend(name);
                let category = self.access.intern_str("DevNet", FindName::Add)?;
                self.access.emit_log(category, &message)?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn parse_fstring(&mut self, text: *const u32, key: &[u32]) -> RuntimeResult<bool> {
        let mut value = [0u32; hp2_core::name::NAME_SIZE];
        Ok(unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), value.as_mut_ptr().cast(), value.len() as i32) })
    }

    fn set_client_third_person(&mut self) -> RuntimeResult<()> {
        let driver = unsafe { ptr::read_unaligned(ptr::addr_of!((*self.pending).base.demo_rec_driver)) }
            .cast::<UDemoRecDriver>();
        unsafe { set_demo_client_third_person(driver) }
    }

    fn set_time_zero(&mut self) -> RuntimeResult<()> {
        let driver = unsafe { ptr::read_unaligned(ptr::addr_of!((*self.pending).base.demo_rec_driver)) }
            .cast::<UNetDriver>();
        unsafe { clear_driver_time(driver) }
    }

    fn set_success(&mut self) {
        unsafe { ptr::addr_of_mut!((*self.pending).base.success).write_unaligned(1); }
    }
}

/// Runs the shared DemoPending text policy directly on canonical Core records.
///
/// # Safety
/// The pending level, connection, and NUL-terminated text are live canonical records.
pub unsafe fn demo_received_text(
    access: CoreAccess<'_>,
    pending: *mut UDemoPlayPendingLevel,
    connection: *mut UNetConnection,
    text: *const u32,
) -> RuntimeResult<()> {
    if pending.is_null() || connection.is_null() || text.is_null() {
        return Err(RuntimeError::new("Invalid canonical demo notification arguments"));
    }
    let mut ops = CoreDemoTextOps { access, pending };
    crate::demo_pending::notify_received_text_policy(&mut ops, connection.cast(), text)
}

fn hex_value(unit: u32) -> Option<u32> {
    match unit {
        48..=57 => Some(unit - 48),
        97..=102 => Some(unit - 97 + 10),
        65..=70 => Some(unit - 65 + 10),
        _ => None,
    }
}

fn c_atoi(text: &[u32]) -> i64 {
    let mut index = 0;
    while text.get(index).is_some_and(|unit| matches!(*unit, 9..=13 | 32)) {
        index += 1;
    }
    let negative = match text.get(index) {
        Some(45) => { index += 1; true }
        Some(43) => { index += 1; false }
        _ => false,
    };
    let mut value = 0i64;
    while let Some(unit @ 48..=57) = text.get(index).copied() {
        value = value.wrapping_mul(10).wrapping_add(i64::from(unit - 48));
        index += 1;
    }
    if negative { value.wrapping_neg() } else { value }
}

fn parse_guid_fields(raw: &[u32; 256], output: &mut [u32; 4]) {
    *output = [0; 4];
    let length = raw.iter().position(|unit| *unit == 0).unwrap_or(raw.len());
    if length != 32 {
        return;
    }
    for (offset, field) in [(24usize, 3usize), (16, 2), (8, 1), (0, 0)] {

        let chunk = &raw[offset..offset + 8];
        let mut index = 0usize;
        while chunk.get(index).is_some_and(|unit| matches!(*unit, 9..=13 | 32)) {
            index += 1;
        }
        let negative = match chunk.get(index) {
            Some(45) => { index += 1; true }
            Some(43) => { index += 1; false }
            _ => false,
        };
        if chunk.get(index) == Some(&48) && matches!(chunk.get(index + 1), Some(120 | 88)) {
            index += 2;
        }
        let mut value = 0u32;
        while let Some(unit) = chunk.get(index).copied() {
            let Some(nibble) = hex_value(unit) else { break; };
            value = value.wrapping_mul(16).wrapping_add(nibble);
            index += 1;
        }
        output[field] = if negative { value.wrapping_neg() } else { value };
    }
}

unsafe fn nul_terminated(text: *const u32) -> RuntimeResult<Vec<u32>> {
    if text.is_null() {
        return Err(RuntimeError::new("Null canonical notification text"));
    }
    let mut length = 0usize;
    while unsafe { *text.add(length) } != 0 { length += 1; }
    Ok(unsafe { std::slice::from_raw_parts(text, length) }.to_vec())
}

fn units_string(units: &[u32]) -> String {
    units.iter().copied().take_while(|unit| *unit != 0)
        .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

fn package_file_exists(access: CoreAccess<'_>, name: &[u32], guid: &[u32; 4]) -> RuntimeResult<bool> {
    hp2_core::package_file_owner::find_package_file(access, name, Some(guid), &mut [0u32; 256])
}


fn wide(value: &str) -> Vec<u32> {
    value.chars().map(u32::from).chain([0]).collect()
}


fn parse_fstring_buffer(text: *const u32, key: &[u32]) -> RuntimeResult<Option<Vec<u32>>> {
    if text.is_null() { return Err(RuntimeError::new("Null canonical text parser input")); }
    let mut text_length = 0usize;
    while unsafe { ptr::read(text.add(text_length)) } != 0 {
        text_length = text_length.checked_add(1)
            .ok_or_else(|| RuntimeError::new("Canonical text length overflow"))?;
    }
    let capacity = text_length.checked_add(1)
        .ok_or_else(|| RuntimeError::new("Canonical text length overflow"))?;
    let parse_capacity = i32::try_from(capacity)
        .map_err(|_| RuntimeError::new("Canonical text exceeds parser capacity"))?;
    let mut value = vec![0; capacity];
    if !unsafe { text_parse::parse_string(text.cast(), key.as_ptr().cast(), value.as_mut_ptr().cast(), parse_capacity) } {
        return Ok(None);
    }
    let length = value.iter().position(|unit| *unit == 0).unwrap_or(value.len());
    value.truncate(length + 1);
    Ok(Some(value))
}

unsafe fn fstring_units(value: TArrayRaw) -> RuntimeResult<Vec<u32>> {
    if value.num < 0 || (value.num != 0 && value.data.is_null()) {
        return Err(RuntimeError::new("Invalid canonical FString storage"));
    }
    if value.num == 0 { return Ok(Vec::new()); }
    let text = unsafe { std::slice::from_raw_parts(value.data.cast::<u32>(), value.num as usize) };
    Ok(text.iter().copied().take_while(|unit| *unit != 0).collect())
}
#[cfg(test)]
mod tests {
    use super::parse_guid_fields;

    #[test]
    fn core_guid_parser_matches_native_field_order() {
        let mut raw = [0u32; 256];
        for (target, unit) in raw.iter_mut().zip("0123456789ABCDEFFEDCBA9876543210".encode_utf16()) {
            *target = u32::from(unit);
        }
        let mut output = [0; 4];
        parse_guid_fields(&raw, &mut output);
        assert_eq!(output, [0x7654_3210, 0xfedc_ba98, 0x89ab_cdef, 0x0123_4567]);
    }
}
