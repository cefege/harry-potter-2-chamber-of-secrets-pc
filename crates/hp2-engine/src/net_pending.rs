//! Rust policy for `Engine/Src/UnPenLev.cpp` (`UNetPendingLevel`).
//!
//! The C++ shell owns Unreal object construction, virtual dispatch, `FString`/
//! `FURL`/`TArray` storage, runtime-format printf, and exception capture. This
//! module owns the client handshake: command parsing and branch selection, the
//! package download bookkeeping, the stats checksum, and every log/command line.
//!
//! Every leaf returns nonzero after capturing a C++ exception; kernels then return
//! `Err(())` and the shell rethrows inside its original `guard`.

use core::{ffi::c_void, mem, ptr};
use std::ffi::{CStr, c_char};

use hp2_abi::engine::generated::{
    APawn, APlayerReplicationInfo, FDownloadInfo, FPackageInfo, FURL, TArrayRaw, UNetConnection,
    UNetDriver, UNetPendingLevel, UPackageMap,
};
use hp2_core::{ansi, command, md5, name::NAME_SIZE};

use crate::level_net::{ACCEPTC_REJECT, ENGINE_MIN_NET_VERSION, ENGINE_VERSION, USOCK_CLOSED};
use hp2_core::native_text::{
    TextPart as A, fmt, fstring_is_empty, fstring_text, fstring_units, scoped,
};
use hp2_core::lit;
use crate::stat_log::encode_utf16le;

/// `PKG_AllowDownload` (UnObjBas.h).
const PKG_ALLOW_DOWNLOAD: u32 = 0x0001;
/// `PKG_Need` (UnObjBas.h).
const PKG_NEED: u32 = 0x8000;
/// `Connection->RemoteVersion <= 420` closes the socket itself (`//!!oldver`).
const OLD_REMOTE_VERSION: i32 = 420;
/// `TCHAR Filename[256]` in the WELCOME package scan.
const FILENAME_LEN: usize = 256;

/// Continuation a scope leaf invokes with the address of its stack object.
pub type Hp2NetPendingContinuation = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

/// Native leaves for `UNetPendingLevel`. Text is `TCHAR` (`u32`); `expr`/`key`
/// are ANSI. Each leaf returns 0, or nonzero after capturing a C++ exception.
#[repr(C)]
pub struct Hp2NetPendingServices {
    /// `debugf(NAME_DevNet, TEXT("%s"), text)`.
    pub log_dev_net: unsafe extern "C" fn(text: *const u32) -> i32,
    /// `debugf(NAME_DevNet, format)`: `format` is runtime (localized) text.
    pub log_dev_net_format: unsafe extern "C" fn(format: *const u32) -> i32,
    /// `appFailAssert(expr, __FILE__, line)` in the shell's translation unit.
    pub check_failed: unsafe extern "C" fn(expr: *const c_char, line: i32) -> i32,
    /// `LocalizeError(key)` in the Engine package.
    pub localize_error: unsafe extern "C" fn(key: *const c_char, result: *mut *const u32) -> i32,
    /// `*(FString*)string = text`.
    pub fstring_assign: unsafe extern "C" fn(string: *mut c_void, text: *const u32) -> i32,
    /// `*(FString*)string = FString::Printf(format, first, second)` with two `%s` texts.
    pub fstring_printf_texts: unsafe extern "C" fn(
        string: *mut c_void,
        format: *const u32,
        first: *const u32,
        second: *const u32,
    ) -> i32,
    /// `appAtoi(text)`.
    pub app_atoi: unsafe extern "C" fn(text: *const u32, result: *mut i32) -> i32,
    /// `Pending->Engine`.
    pub pending_engine: unsafe extern "C" fn(pending: *mut c_void, result: *mut *mut c_void) -> i32,
    /// `&Pending->URL`.
    pub pending_url: unsafe extern "C" fn(pending: *mut c_void, result: *mut *mut c_void) -> i32,
    /// `StaticLoadClass(UNetDriver::StaticClass(), NULL, name, NULL, LOAD_NoFail, NULL)`.
    pub static_load_driver_class:
        unsafe extern "C" fn(name: *const u32, result: *mut *mut c_void) -> i32,
    /// `ConstructObject<UNetDriver>(cls)`.
    pub construct_driver: unsafe extern "C" fn(cls: *mut c_void, result: *mut *mut c_void) -> i32,
    /// `Driver->InitConnect(Pending, Pending->URL, Pending->Error)`, passing the
    /// pending level's `FNetworkNotify` base.
    pub driver_init_connect:
        unsafe extern "C" fn(driver: *mut c_void, pending: *mut c_void, result: *mut i32) -> i32,
    /// `delete Driver`.
    pub driver_delete: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    /// `Driver->TickDispatch(delta)`.
    pub driver_tick_dispatch: unsafe extern "C" fn(driver: *mut c_void, delta: f32) -> i32,
    /// `Driver->TickFlush()`.
    pub driver_tick_flush: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    /// `Connection->Logf(TEXT("%s"), text)`.
    pub connection_log: unsafe extern "C" fn(conn: *mut c_void, text: *const u32) -> i32,
    /// `Connection->FlushNet()`.
    pub connection_flush_net: unsafe extern "C" fn(conn: *mut c_void) -> i32,
    /// `Connection->ReceiveFile(index)`.
    pub connection_receive_file: unsafe extern "C" fn(conn: *mut c_void, index: i32) -> i32,
    /// `Channel->Close()`.
    pub channel_close: unsafe extern "C" fn(channel: *mut c_void) -> i32,
    /// `Download->TrySkipFile()`.
    pub download_try_skip_file:
        unsafe extern "C" fn(download: *mut c_void, result: *mut i32) -> i32,
    /// `delete Download`.
    pub download_delete: unsafe extern "C" fn(download: *mut c_void) -> i32,
    /// `Connection->DownloadInfo.AddZeroed()`.
    pub download_info_add_zeroed: unsafe extern "C" fn(conn: *mut c_void, result: *mut i32) -> i32,
    /// `Connection->DownloadInfo.Remove(index)`.
    pub download_info_remove: unsafe extern "C" fn(conn: *mut c_void, index: i32) -> i32,
    /// `StaticLoadClass(UDownload::StaticClass(), NULL, name, NULL, LOAD_NoWarn | LOAD_Quiet, NULL)`.
    pub load_download_class:
        unsafe extern "C" fn(name: *const u32, result: *mut *mut c_void) -> i32,
    /// `new(Map->List)FPackageInfo(NULL)`; yields the new index.
    pub package_list_add: unsafe extern "C" fn(map: *mut c_void, result: *mut i32) -> i32,
    /// `Map->List.Remove(index)`.
    pub package_list_remove: unsafe extern "C" fn(map: *mut c_void, index: i32) -> i32,
    /// `CreatePackage(NULL, name)`.
    pub create_package: unsafe extern "C" fn(name: *const u32, result: *mut *mut c_void) -> i32,
    /// `Object->GetName()`.
    pub object_name: unsafe extern "C" fn(object: *mut c_void, result: *mut *const u32) -> i32,
    /// `appFindPackageFile(name, guid, filename, max)`.
    pub find_package_file: unsafe extern "C" fn(
        name: *const u32,
        guid: *const c_void,
        filename: *mut u32,
        max: i32,
        result: *mut i32,
    ) -> i32,
    /// `DLLEXT`.
    pub dll_ext: unsafe extern "C" fn(result: *mut *const u32) -> i32,
    /// `GFileManager->FileSize(filename)`.
    pub file_size: unsafe extern "C" fn(filename: *const u32, result: *mut i32) -> i32,
    /// `Parse(text, key, INT&)`.
    pub parse_int: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        value: *mut i32,
        parsed: *mut i32,
    ) -> i32,
    /// `Parse(text, key, DWORD&)`.
    pub parse_dword: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        value: *mut u32,
        parsed: *mut i32,
    ) -> i32,
    /// `Parse(text, key, FGuid&)`.
    pub parse_guid: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        value: *mut c_void,
        parsed: *mut i32,
    ) -> i32,
    /// `Parse(text, key, TCHAR*, max)`.
    pub parse_text: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        value: *mut u32,
        max: i32,
        parsed: *mut i32,
    ) -> i32,
    /// `Parse(text, key, FString&)`.
    pub parse_fstring: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        value: *mut c_void,
        parsed: *mut i32,
    ) -> i32,
    /// `ParseUBOOL(text, key, UBOOL&)`.
    pub parse_ubool: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        value: *mut i32,
        parsed: *mut i32,
    ) -> i32,
    /// `FURL Copy(*url)`; resumes with `&Copy`, destroyed when the scope ends.
    pub url_copy_scope: unsafe extern "C" fn(
        url: *mut c_void,
        context: *mut c_void,
        resume: Hp2NetPendingContinuation,
    ) -> i32,
    /// `FString Text = url->String()`; resumes with `*Text`.
    pub url_string_scope: unsafe extern "C" fn(
        url: *mut c_void,
        context: *mut c_void,
        resume: Hp2NetPendingContinuation,
    ) -> i32,
    /// `*(FString*)string = url->String()`.
    pub url_string_assign: unsafe extern "C" fn(url: *mut c_void, string: *mut c_void) -> i32,
    /// `url->Op.Remove(index)`.
    pub url_op_remove: unsafe extern "C" fn(url: *mut c_void, index: i32) -> i32,
    /// `url->AddOption(text)`.
    pub url_add_option: unsafe extern "C" fn(url: *mut c_void, text: *const u32) -> i32,
    /// `Engine->ChallengeResponse(challenge)`.
    pub engine_challenge_response:
        unsafe extern "C" fn(engine: *mut c_void, challenge: i32, result: *mut i32) -> i32,
    /// `Engine->Client->Viewports(0)->Actor`.
    pub engine_viewport_actor:
        unsafe extern "C" fn(engine: *mut c_void, result: *mut *mut c_void) -> i32,
    /// `&PlayerPawn->ngWorldSecret`.
    pub pawn_world_secret: unsafe extern "C" fn(pawn: *mut c_void, result: *mut *mut c_void) -> i32,
    /// Raises `appMD5UpdateStringUtf16LE`'s 32-bit length failure.
    pub md5_overflow: unsafe extern "C" fn() -> i32,
}

macro_rules! value {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = mem::MaybeUninit::uninit();
        if ($callback)($($arg,)* out.as_mut_ptr()) != 0 { return Err(()); }
        out.assume_init()
    }};
}
macro_rules! action {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        if ($callback)($($arg),*) != 0 { return Err(()); }
    }};
}

type R<T> = core::result::Result<T, ()>;

// ---------------------------------------------------------------------------
// Layout access
// ---------------------------------------------------------------------------

unsafe fn net_driver_slot(this: *mut c_void) -> *mut *mut c_void {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.net_driver) }
}
unsafe fn net_driver(this: *mut c_void) -> *mut c_void {
    unsafe { ptr::read_unaligned(net_driver_slot(this)) }
}
unsafe fn error_slot(this: *mut c_void) -> *mut TArrayRaw {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.error) }
}
unsafe fn fail_code_slot(this: *mut c_void) -> *mut TArrayRaw {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.fail_code) }
}
unsafe fn fail_url_slot(this: *mut c_void) -> *mut TArrayRaw {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.fail_url) }
}
unsafe fn success_slot(this: *mut c_void) -> *mut i32 {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.success) }
}
unsafe fn sent_join_slot(this: *mut c_void) -> *mut i32 {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.sent_join) }
}
unsafe fn lone_player_slot(this: *mut c_void) -> *mut i32 {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.lone_player) }
}
unsafe fn files_needed_slot(this: *mut c_void) -> *mut i32 {
    unsafe { ptr::addr_of_mut!((*this.cast::<UNetPendingLevel>()).base.files_needed) }
}

/// `NetDriver->ServerConnection`.
unsafe fn server_connection(driver: *mut c_void) -> *mut c_void {
    unsafe {
        ptr::read_unaligned(ptr::addr_of!(
            (*driver.cast::<UNetDriver>()).server_connection
        ))
    }
}
/// `conn` as its layout mirror.
fn connection(conn: *mut c_void) -> *mut UNetConnection {
    conn.cast()
}
/// `Connection->PackageMap`.
unsafe fn package_map(conn: *mut c_void) -> *mut c_void {
    unsafe { ptr::read_unaligned(ptr::addr_of!((*connection(conn)).package_map)) }
}
/// `Map->List`.
unsafe fn package_list(map: *mut c_void) -> TArrayRaw {
    unsafe { ptr::read_unaligned(ptr::addr_of!((*map.cast::<UPackageMap>()).list)) }
}
/// `&Map->List(index)`.
unsafe fn package_info(map: *mut c_void, index: i32) -> *mut FPackageInfo {
    unsafe {
        package_list(map)
            .data
            .cast::<FPackageInfo>()
            .add(index as usize)
    }
}
unsafe fn package_flags_slot(info: *mut FPackageInfo) -> *mut u32 {
    unsafe { ptr::addr_of_mut!((*info).package_flags) }
}
unsafe fn package_parent(info: *mut FPackageInfo) -> *mut c_void {
    unsafe { ptr::read_unaligned(ptr::addr_of!((*info).parent)) }
}
/// `&Connection->DownloadInfo(index)`.
unsafe fn download_info(conn: *mut c_void, index: i32) -> *mut FDownloadInfo {
    unsafe {
        let infos = ptr::read_unaligned(ptr::addr_of!((*connection(conn)).download_info));
        infos.data.cast::<FDownloadInfo>().add(index as usize)
    }
}
/// `&Url->Op`.
unsafe fn url_ops(url: *mut c_void) -> *mut TArrayRaw {
    unsafe { ptr::addr_of_mut!((*url.cast::<FURL>()).op) }
}

/// `Url->Op(index).Left(prefix length) == prefix`. `FString::operator==` is
/// `appStricmp`, so the original `Caps()` on the password test cannot change
/// the result: it folds ASCII letters only, and the comparison already does.
unsafe fn op_has_prefix(url: *mut c_void, index: i32, prefix: &[u32]) -> bool {
    const MAX_PREFIX: usize = 16;
    let count = prefix.len() - 1;
    assert!(count < MAX_PREFIX);
    let mut left = [0u32; MAX_PREFIX];
    unsafe {
        let ops = ptr::read_unaligned(url_ops(url));
        let text = fstring_text(ops.data.cast::<TArrayRaw>().add(index as usize));
        for (i, unit) in left.iter_mut().take(count).enumerate() {
            let ch = *text.add(i);
            if ch == 0 {
                break;
            }
            *unit = ch;
        }
        ansi::stricmp(left.as_ptr().cast(), prefix.as_ptr().cast()) == 0
    }
}

/// `check(expr)`: the leaf raises; a failed check never continues.
fn check(s: &Hp2NetPendingServices, expr: &CStr, line: i32, failed: bool) -> R<()> {
    if failed {
        unsafe { action!(s.check_failed, expr.as_ptr(), line) };
        return Err(());
    }
    Ok(())
}

/// `*string = LocalizeError(key)`.
unsafe fn assign_localized_error(
    s: &Hp2NetPendingServices,
    string: *mut TArrayRaw,
    key: &CStr,
) -> R<()> {
    unsafe {
        let text = value!(s.localize_error, key.as_ptr());
        action!(s.fstring_assign, string.cast(), text);
    }
    Ok(())
}

unsafe fn parse_command(text: &mut *const u32, command: &[u32]) -> bool {
    unsafe { command::parse_command(ptr::from_mut(text).cast(), command.as_ptr().cast()) != 0 }
}

/// The ngWorldStats option. Same policy as `AStatLog::GetPlayerChecksum`: no pawn
/// or an empty secret yields `NoChecksum`; no `PlayerReplicationInfo` hashes an
/// empty name. Otherwise MD5 over UTF-16LE name then secret, as `%02x` pairs.
unsafe fn checksum_option(s: &Hp2NetPendingServices, pawn: *mut c_void) -> R<Vec<u32>> {
    let secret = if pawn.is_null() {
        ptr::null()
    } else {
        unsafe { value!(s.pawn_world_secret, pawn) }
            .cast::<TArrayRaw>()
            .cast_const()
    };
    if secret.is_null() || unsafe { fstring_is_empty(secret) } {
        return Ok(lit!("Checksum=NoChecksum").to_vec());
    }
    let pri = unsafe {
        ptr::read_unaligned(ptr::addr_of!(
            (*pawn.cast::<APawn>()).player_replication_info
        ))
    };
    let name = if pri.is_null() {
        &[][..]
    } else {
        unsafe {
            fstring_units(ptr::addr_of!(
                (*pri.cast::<APlayerReplicationInfo>()).player_name
            ))
        }
    };
    let mut context = mem::MaybeUninit::<md5::Md5Context>::uninit();
    unsafe { md5::init(context.as_mut_ptr()) };
    for text in [name, unsafe { fstring_units(secret) }] {
        let Some(bytes) = encode_utf16le(text, false) else {
            unsafe { action!(s.md5_overflow) };
            return Err(());
        };
        unsafe { md5::update(context.as_mut_ptr(), bytes.as_ptr(), bytes.len() as i32) };
    }
    let mut digest = [0u8; 16];
    unsafe { md5::final_digest(digest.as_mut_ptr(), context.as_mut_ptr()) };
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut option = lit!("Checksum=")[..9].to_vec();
    for byte in digest {
        option.push(HEX[usize::from(byte >> 4)].into());
        option.push(HEX[usize::from(byte & 15)].into());
    }
    option.push(0);
    Ok(option)
}

// ---------------------------------------------------------------------------
// UNetPendingLevel
// ---------------------------------------------------------------------------

/// `UNetPendingLevel::UNetPendingLevel`: create the configured network driver and
/// send HELLO, or discard a driver whose connect fails.
///
/// # Safety
/// `this` must be a `UNetPendingLevel` under construction with its base built;
/// the services must be live.
pub unsafe fn constructor(s: &Hp2NetPendingServices, this: *mut c_void) -> R<()> {
    unsafe {
        action!(s.fstring_assign, error_slot(this).cast(), lit!("").as_ptr());
        ptr::write_unaligned(net_driver_slot(this), ptr::null_mut());
        let class = value!(
            s.static_load_driver_class,
            lit!("ini:Engine.Engine.NetworkDevice").as_ptr(),
        );
        let driver = value!(s.construct_driver, class);
        ptr::write_unaligned(net_driver_slot(this), driver);
        if value!(s.driver_init_connect, net_driver(this), this) != 0 {
            let hello = fmt(&[
                A::L("HELLO REVISION=0 MINVER="),
                A::I(ENGINE_MIN_NET_VERSION),
                A::L(" VER="),
                A::I(ENGINE_VERSION),
            ]);
            action!(
                s.connection_log,
                server_connection(net_driver(this)),
                hello.as_ptr()
            );
            action!(s.connection_flush_net, server_connection(net_driver(this)));
        } else {
            action!(s.driver_delete, net_driver(this));
            ptr::write_unaligned(net_driver_slot(this), ptr::null_mut());
        }
    }
    Ok(())
}

/// `NotifyAcceptingConnection`: a pending client accepts no connections.
pub fn notify_accepting_connection() -> i32 {
    ACCEPTC_REJECT
}

/// `NotifyAcceptingChannel`: a pending client accepts no channels.
pub fn notify_accepting_channel() -> i32 {
    0
}

/// `NotifyGetLevel`: a pending level has no level.
pub fn notify_get_level() -> *mut c_void {
    ptr::null_mut()
}

/// Defines the low-level operations used by the shared pending text handshake.
pub(crate) trait NotifyReceivedTextOps {
    type Error;
    fn check_connection(&mut self, this: *mut c_void, conn: *mut c_void) -> Result<(), Self::Error>;
    fn log_received_text(&mut self, text: *const u32) -> Result<(), Self::Error>;
    fn log_welcome(&mut self, text: *const u32) -> Result<(), Self::Error>;
    fn parse_command(&mut self, text: &mut *const u32, command: &[u32]) -> Result<bool, Self::Error>;
    fn parse_int(&mut self, text: *const u32, key: &[u32], value: &mut i32) -> Result<(), Self::Error>;
    fn parse_ubool(&mut self, text: *const u32, key: &[u32], value: &mut i32) -> Result<(), Self::Error>;
    fn parse_dword(&mut self, text: *const u32, key: &[u32], value: &mut u32) -> Result<(), Self::Error>;
    fn parse_guid(&mut self, text: *const u32, key: &[u32], value: &mut [u32; 4]) -> Result<(), Self::Error>;
    fn parse_text(&mut self, text: *const u32, key: &[u32], value: &mut [u32; NAME_SIZE]) -> Result<(), Self::Error>;
    fn parse_fstring(&mut self, text: *const u32, key: &[u32], destination: *mut TArrayRaw) -> Result<(), Self::Error>;
    fn parse_atoi(&mut self, text: *const u32) -> Result<i32, Self::Error>;
    fn localize_error(&mut self, key: &CStr) -> Result<*const u32, Self::Error>;
    fn assign_fstring(&mut self, destination: *mut TArrayRaw, text: *const u32) -> Result<(), Self::Error>;
    fn pending_url(&mut self, this: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn url_op_count(&mut self, url: *mut c_void) -> i32;
    fn url_op_has_prefix(&mut self, url: *mut c_void, index: i32, prefix: &[u32]) -> bool;
    fn remove_url_op(&mut self, url: *mut c_void, index: i32) -> Result<(), Self::Error>;
    fn url_string_assign(&mut self, url: *mut c_void, destination: *mut TArrayRaw) -> Result<(), Self::Error>;
    fn with_url_copy<F>(&mut self, url: *mut c_void, body: F) -> Result<(), Self::Error>
    where F: FnOnce(&mut Self, *mut c_void) -> Result<(), Self::Error>;
    fn with_url_string<F>(&mut self, url: *mut c_void, body: F) -> Result<(), Self::Error>
    where F: FnOnce(&mut Self, *const u32) -> Result<(), Self::Error>;
    fn add_url_option(&mut self, url: *mut c_void, text: *const u32) -> Result<(), Self::Error>;
    fn create_package(&mut self, name: &[u32]) -> Result<*mut c_void, Self::Error>;
    fn append_package_info(&mut self, map: *mut c_void) -> Result<*mut FPackageInfo, Self::Error>;
    fn stats_checksum(&mut self, this: *mut c_void) -> Result<Vec<u32>, Self::Error>;
    fn log_connection(&mut self, conn: *mut c_void, text: *const u32) -> Result<(), Self::Error>;
    fn challenge_response(&mut self, this: *mut c_void, challenge: i32) -> Result<i32, Self::Error>;
    fn flush_connection(&mut self, conn: *mut c_void) -> Result<(), Self::Error>;
    fn add_download_info(&mut self, conn: *mut c_void) -> Result<(i32, *mut FDownloadInfo), Self::Error>;
    fn load_download_class(&mut self, name: *const u32) -> Result<*mut c_void, Self::Error>;
    fn remove_download_info(&mut self, conn: *mut c_void, index: i32) -> Result<(), Self::Error>;
    fn package_name(&mut self, info: *mut FPackageInfo) -> Result<*const u32, Self::Error>;
    fn find_package_file(&mut self, name: *const u32, guid: *const c_void, filename: &mut [u32; FILENAME_LEN]) -> Result<bool, Self::Error>;
    fn dll_extension(&mut self) -> Result<*const u32, Self::Error>;
    fn file_size(&mut self, filename: *const u32) -> Result<i32, Self::Error>;
    fn allow_downloads(&mut self, this: *mut c_void) -> bool;
    fn close_first_channel(&mut self, conn: *mut c_void) -> Result<(), Self::Error>;
    fn set_success(&mut self, this: *mut c_void);
    fn package_map(&mut self, conn: *mut c_void) -> *mut c_void;
    fn package_count(&mut self, map: *mut c_void) -> i32;
    fn package_at(&mut self, map: *mut c_void, index: i32) -> *mut FPackageInfo;
    fn package_flags(&mut self, info: *mut FPackageInfo) -> u32;
    fn request_file(&mut self, conn: *mut c_void, index: i32) -> Result<(), Self::Error>;
    fn delete_finished_download(&mut self, conn: *mut c_void) -> Result<(), Self::Error>;
    fn error_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw;
    fn fail_code_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw;
    fn fail_url_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw;
    fn user_flags_slot(&mut self, conn: *mut c_void) -> *mut i32;
    fn negotiated_version_slot(&mut self, conn: *mut c_void) -> *mut i32;
    fn challenge_slot(&mut self, conn: *mut c_void) -> *mut i32;
    fn current_speed(&mut self, conn: *mut c_void) -> i32;
    fn remote_version(&mut self, conn: *mut c_void) -> i32;
    fn close_old_connection(&mut self, conn: *mut c_void);
    fn set_package_parent(&mut self, info: *mut FPackageInfo, parent: *mut c_void);
    fn set_package_flags(&mut self, info: *mut FPackageInfo, flags: u32);
    fn increment_files_needed(&mut self, this: *mut c_void);
    fn url_host_slot(&mut self, url: *mut c_void) -> *mut TArrayRaw;
    fn server_connection(&mut self, this: *mut c_void) -> *mut c_void;
    fn url_map_slot(&mut self, url: *mut c_void) -> *mut TArrayRaw;
    fn download_class_name(&mut self, info: *mut FDownloadInfo) -> *const u32;
    fn set_download_class(&mut self, info: *mut FDownloadInfo, class: *mut c_void);
    fn package_guid_slot(&mut self, info: *mut FPackageInfo) -> *mut [u32; 4];
    fn package_generation_slot(&mut self, info: *mut FPackageInfo) -> *mut i32;
    fn package_size_slot(&mut self, info: *mut FPackageInfo) -> *mut i32;
    fn package_filename_slot(&mut self, info: *mut FPackageInfo) -> *mut TArrayRaw;
    fn download_class_name_slot(&mut self, info: *mut FDownloadInfo) -> *mut TArrayRaw;
    fn download_params_slot(&mut self, info: *mut FDownloadInfo) -> *mut TArrayRaw;
    fn download_compression_slot(&mut self, info: *mut FDownloadInfo) -> *mut i32;
    fn package_flags_slot(&mut self, info: *mut FPackageInfo) -> *mut u32;
    fn lone_player_slot(&mut self, this: *mut c_void) -> *mut i32;
}

/// Runs the shared policy for a received network-text message.
///
/// # Safety
/// The pointers and operations must satisfy the native callback contracts.
pub(crate) unsafe fn notify_received_text_with<O: NotifyReceivedTextOps>(
    ops: &mut O, this: *mut c_void, conn: *mut c_void, text0: *const u32,
) -> Result<(), O::Error> {
    ops.check_connection(this, conn)?;
    ops.log_received_text(text0)?;
    let mut text = text0;
    if ops.parse_command(&mut text, lit!("UPGRADE"))? {
        let mut remote_min_ver = 0;
        let mut remote_ver = 0;
        ops.parse_int(text, lit!("MINVER="), &mut remote_min_ver)?;
        ops.parse_int(text, lit!("VER="), &mut remote_ver)?;
        let key = if ENGINE_VERSION < remote_min_ver { c"ClientOutdated" } else { c"ServerOutdated" };
        let error = ops.localize_error(key)?;
        let destination = ops.error_slot(this);
        ops.assign_fstring(destination, error)?;
    } else if ops.parse_command(&mut text, lit!("FAILURE"))? {
        let error = ops.localize_error(c"RejectedByServer")?;
        let destination = ops.error_slot(this);
        ops.assign_fstring(destination, error)?;
    } else if ops.parse_command(&mut text, lit!("FAILCODE"))? {
        let url = ops.pending_url(this)?;
        ops.with_url_copy(url, |ops, copy| {
            for i in (0..ops.url_op_count(copy)).rev() {
                if ops.url_op_has_prefix(copy, i, lit!("PASSWORD=")) {
                    ops.remove_url_op(copy, i)?;
                }
            }
            let fail_code = ops.fail_code_slot(this);
            ops.assign_fstring(fail_code, text)?;
            let fail_url = ops.fail_url_slot(this);
            ops.url_string_assign(copy, fail_url)
        })?;
    } else if ops.parse_command(&mut text, lit!("USES"))? {
        let map = ops.package_map(conn);
        let info = ops.append_package_info(map)?;
        let guid_slot = ops.package_guid_slot(info);
        ops.parse_guid(text, lit!("GUID="), unsafe { &mut *guid_slot })?;
        let generation_slot = ops.package_generation_slot(info);
        ops.parse_int(text, lit!("GEN="), unsafe { &mut *generation_slot })?;
        let size_slot = ops.package_size_slot(info);
        ops.parse_int(text, lit!("SIZE="), unsafe { &mut *size_slot })?;
        let flags_slot = ops.package_flags_slot(info);
        ops.parse_dword(text, lit!("FLAGS="), unsafe { &mut *flags_slot })?;
        let mut name = [0; NAME_SIZE];
        ops.parse_text(text, lit!("PKG="), &mut name)?;
        let filename_slot = ops.package_filename_slot(info);
        ops.parse_fstring(text, lit!("FNAME="), filename_slot)?;
        let parent = ops.create_package(&name)?;
        ops.set_package_parent(info, parent);
    } else if ops.parse_command(&mut text, lit!("USERFLAG"))? {
        let flags = ops.parse_atoi(text)?;
        unsafe { ptr::write_unaligned(ops.user_flags_slot(conn), flags); }
    } else if ops.parse_command(&mut text, lit!("CHALLENGE"))? {
        let version = ops.negotiated_version_slot(conn);
        ops.parse_int(text, lit!("VER="), unsafe { &mut *version })?;
        let challenge = ops.challenge_slot(conn);
        ops.parse_int(text, lit!("CHALLENGE="), unsafe { &mut *challenge })?;
        let mut remote_stats = 0;
        ops.parse_int(text, lit!("STATS="), &mut remote_stats)?;
        let url = ops.pending_url(this)?;
        if remote_stats == 1 {
            let option = ops.stats_checksum(this)?;
            ops.add_url_option(url, option.as_ptr())?;
        }
        ops.with_url_copy(url, |ops, partial| {
            let host = ops.url_host_slot(partial);
            ops.assign_fstring(host, lit!("").as_ptr())?;
            for i in (0..ops.url_op_count(partial)).rev() {
                if ops.url_op_has_prefix(partial, i, lit!("game=")) {
                    ops.remove_url_op(partial, i)?;
                }
            }
            let server = ops.server_connection(this);
            let speed = fmt(&[A::L("NETSPEED "), A::I(ops.current_speed(conn))]);
            ops.log_connection(server, speed.as_ptr())?;
            let challenge = unsafe { ptr::read_unaligned(ops.challenge_slot(conn)) };
            let response = ops.challenge_response(this, challenge)?;
            ops.with_url_string(partial, |ops, partial_text| {
                let login = fmt(&[A::L("LOGIN RESPONSE="), A::I(response), A::L(" URL="), A::P(partial_text)]);
                ops.log_connection(server, login.as_ptr())
            })?;
            ops.flush_connection(server)
        })?;
    } else if ops.parse_command(&mut text, lit!("DLMGR"))? {
        // Source order: add the zeroed record, then parse into its FStrings.
        let (index, info) = ops.add_download_info(conn)?;
        let class_slot = ops.download_class_name_slot(info);
        ops.parse_fstring(text, lit!("CLASS="), class_slot)?;
        let params_slot = ops.download_params_slot(info);
        ops.parse_fstring(text, lit!("PARAMS="), params_slot)?;
        let mut compression = 0;
        ops.parse_ubool(text, lit!("COMPRESSION="), &mut compression)?;
        let compression_slot = ops.download_compression_slot(info);
        unsafe { ptr::write_unaligned(compression_slot, compression); }
        let class_name = ops.download_class_name(info);
        let class = ops.load_download_class(class_name)?;
        ops.set_download_class(info, class);
        if class.is_null() { ops.remove_download_info(conn, index)?; }
    } else if ops.parse_command(&mut text, lit!("WELCOME"))? {
        ops.log_welcome(text)?;
        let url = ops.pending_url(this)?;
        let map_slot = ops.url_map_slot(url);
        ops.parse_fstring(text, lit!("LEVEL="), map_slot)?;
        let lone = ops.lone_player_slot(this);
        ops.parse_ubool(text, lit!("LONE="), unsafe { &mut *lone })?;
        let challenge = ops.challenge_slot(conn);
        ops.parse_int(text, lit!("CHALLENGE="), unsafe { &mut *challenge })?;
        let map = ops.package_map(conn);
        let mut index = 0;
        while index < ops.package_count(map) {
            let info = ops.package_at(map, index);
            let name = ops.package_name(info)?;
            let mut filename = [0; FILENAME_LEN];
            let guid = ops.package_guid_slot(info).cast();
            let found = ops.find_package_file(name, guid, &mut filename)?;
            if !found {
                let ext = ops.dll_extension()?;
                let full_name = fmt(&[A::P(name), A::P(ext)]);
                if ops.file_size(full_name.as_ptr())? <= 0 {
                    ops.increment_files_needed(this);
                    let flags = ops.package_flags(info) | PKG_NEED;
                    ops.set_package_flags(info, flags);
                    if !ops.allow_downloads(this) || flags & PKG_ALLOW_DOWNLOAD == 0 {
                        let error = fmt(&[A::L("Downloading '"), A::P(name), A::L("' not allowed")]);
                        let slot = ops.error_slot(this);
                        ops.assign_fstring(slot, error.as_ptr())?;
                        ops.close_first_channel(conn)?;
                        ops.flush_connection(conn)?;
                        if ops.remote_version(conn) <= OLD_REMOTE_VERSION { ops.close_old_connection(conn); }
                        return Ok(());
                    }
                }
            }
            index += 1;
        }
        let map = ops.package_map(conn);
        let mut index = 0;
        while index < ops.package_count(map) {
            let info = ops.package_at(map, index);
            if ops.package_flags(info) & PKG_NEED != 0 {
                ops.request_file(conn, index)?;
                break;
            }
            index += 1;
        }
        if index == ops.package_count(map) {
            ops.delete_finished_download(conn)?;
        }
        ops.set_success(this);
    }
    Ok(())
}

/// `UNetPendingLevel::NotifyReceivedText`: the client handshake.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel` with a live `NetDriver`, `conn` a live
/// `UNetConnection`, and `text` NUL-terminated.
pub unsafe fn notify_received_text(
    s: &Hp2NetPendingServices,
    this: *mut c_void,
    conn: *mut c_void,
    text: *const u32,
) -> R<()> {
    unsafe { notify_received_text_with(&mut NativeNotifyReceivedTextOps(s), this, conn, text) }
}

struct NativeNotifyReceivedTextOps<'a>(&'a Hp2NetPendingServices);
impl NotifyReceivedTextOps for NativeNotifyReceivedTextOps<'_> {
    type Error = ();
    fn check_connection(&mut self, this: *mut c_void, conn: *mut c_void) -> R<()> {
        unsafe { check(self.0, c"Connection==NetDriver->ServerConnection", 87, conn != server_connection(net_driver(this))) }
    }
    fn log_received_text(&mut self, text: *const u32) -> R<()> {
        unsafe { let line = fmt(&[A::L("PendingLevel received: "), A::P(text)]); action!(self.0.log_dev_net, line.as_ptr()); }
        Ok(())
    }
    fn log_welcome(&mut self, text: *const u32) -> R<()> {
        unsafe { let line = fmt(&[A::L("Welcomed by server: "), A::P(text)]); action!(self.0.log_dev_net, line.as_ptr()); }
        Ok(())
    }
    fn parse_command(&mut self, text: &mut *const u32, command: &[u32]) -> R<bool> { Ok(unsafe { parse_command(text, command) }) }
    fn parse_int(&mut self, text: *const u32, key: &[u32], value: &mut i32) -> R<()> { unsafe { value!(self.0.parse_int, text, key.as_ptr(), value); } Ok(()) }
    fn parse_dword(&mut self, text: *const u32, key: &[u32], value: &mut u32) -> R<()> { unsafe { value!(self.0.parse_dword, text, key.as_ptr(), value); } Ok(()) }
    fn parse_guid(&mut self, text: *const u32, key: &[u32], value: &mut [u32; 4]) -> R<()> { unsafe { value!(self.0.parse_guid, text, key.as_ptr(), value.as_mut_ptr().cast()); } Ok(()) }
    fn parse_text(&mut self, text: *const u32, key: &[u32], value: &mut [u32; NAME_SIZE]) -> R<()> { unsafe { value!(self.0.parse_text, text, key.as_ptr(), value.as_mut_ptr(), NAME_SIZE as i32); } Ok(()) }
    fn parse_fstring(&mut self, text: *const u32, key: &[u32], destination: *mut TArrayRaw) -> R<()> {
        unsafe { value!(self.0.parse_fstring, text, key.as_ptr(), destination.cast()); }
        Ok(())
    }
    fn parse_ubool(&mut self, text: *const u32, key: &[u32], result: &mut i32) -> R<()> {
        unsafe { value!(self.0.parse_ubool, text, key.as_ptr(), result); }
        Ok(())
    }
    fn parse_atoi(&mut self, text: *const u32) -> R<i32> { unsafe { Ok(value!(self.0.app_atoi, text)) } }
    fn localize_error(&mut self, key: &CStr) -> R<*const u32> { unsafe { Ok(value!(self.0.localize_error, key.as_ptr())) } }
    fn assign_fstring(&mut self, destination: *mut TArrayRaw, text: *const u32) -> R<()> { unsafe { action!(self.0.fstring_assign, destination.cast(), text); } Ok(()) }
    fn pending_url(&mut self, this: *mut c_void) -> R<*mut c_void> { unsafe { Ok(value!(self.0.pending_url, this)) } }
    fn url_op_count(&mut self, url: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(url_ops(url)).num } }
    fn url_op_has_prefix(&mut self, url: *mut c_void, index: i32, prefix: &[u32]) -> bool { unsafe { op_has_prefix(url, index, prefix) } }
    fn remove_url_op(&mut self, url: *mut c_void, index: i32) -> R<()> { unsafe { action!(self.0.url_op_remove, url, index); } Ok(()) }
    fn url_string_assign(&mut self, url: *mut c_void, destination: *mut TArrayRaw) -> R<()> { unsafe { action!(self.0.url_string_assign, url, destination.cast()); } Ok(()) }
    fn with_url_copy<F>(&mut self, url: *mut c_void, body: F) -> R<()>
    where F: FnOnce(&mut Self, *mut c_void) -> R<()> {
        let services = self.0;
        let ops = self as *mut Self;
        unsafe { scoped(|context, resume| (services.url_copy_scope)(url, context, resume), |copy| body(&mut *ops, copy)) }
    }
    fn with_url_string<F>(&mut self, url: *mut c_void, body: F) -> R<()>
    where F: FnOnce(&mut Self, *const u32) -> R<()> {
        let services = self.0;
        let ops = self as *mut Self;
        unsafe { scoped(|context, resume| (services.url_string_scope)(url, context, resume), |text| body(&mut *ops, text.cast_const().cast())) }
    }
    fn add_url_option(&mut self, url: *mut c_void, text: *const u32) -> R<()> { unsafe { action!(self.0.url_add_option, url, text); } Ok(()) }
    fn create_package(&mut self, name: &[u32]) -> R<*mut c_void> { unsafe { Ok(value!(self.0.create_package, name.as_ptr())) } }
    fn append_package_info(&mut self, map: *mut c_void) -> R<*mut FPackageInfo> {
        unsafe { let index = value!(self.0.package_list_add, map); Ok(package_info(map, index)) }
    }
    fn stats_checksum(&mut self, this: *mut c_void) -> R<Vec<u32>> {
        unsafe {
            let engine = value!(self.0.pending_engine, this);
            let pawn = value!(self.0.engine_viewport_actor, engine);
            checksum_option(self.0, pawn)
        }
    }
    fn log_connection(&mut self, conn: *mut c_void, text: *const u32) -> R<()> { unsafe { action!(self.0.connection_log, conn, text); } Ok(()) }
    fn challenge_response(&mut self, this: *mut c_void, challenge: i32) -> R<i32> { unsafe { Ok(value!(self.0.engine_challenge_response, value!(self.0.pending_engine, this), challenge)) } }
    fn flush_connection(&mut self, conn: *mut c_void) -> R<()> { unsafe { action!(self.0.connection_flush_net, conn); } Ok(()) }
    fn add_download_info(&mut self, conn: *mut c_void) -> R<(i32, *mut FDownloadInfo)> {
        unsafe { let index = value!(self.0.download_info_add_zeroed, conn); Ok((index, download_info(conn, index))) }
    }
    fn load_download_class(&mut self, name: *const u32) -> R<*mut c_void> { unsafe { Ok(value!(self.0.load_download_class, name)) } }
    fn remove_download_info(&mut self, conn: *mut c_void, index: i32) -> R<()> { unsafe { action!(self.0.download_info_remove, conn, index); } Ok(()) }
    fn package_name(&mut self, info: *mut FPackageInfo) -> R<*const u32> { unsafe { Ok(value!(self.0.object_name, package_parent(info))) } }
    fn find_package_file(&mut self, name: *const u32, guid: *const c_void, filename: &mut [u32; FILENAME_LEN]) -> R<bool> {
        unsafe { Ok(value!(self.0.find_package_file, name, guid, filename.as_mut_ptr(), FILENAME_LEN as i32) != 0) }
    }
    fn dll_extension(&mut self) -> R<*const u32> { unsafe { Ok(value!(self.0.dll_ext)) } }
    fn file_size(&mut self, filename: *const u32) -> R<i32> { unsafe { Ok(value!(self.0.file_size, filename)) } }
    fn allow_downloads(&mut self, this: *mut c_void) -> bool { unsafe { ptr::read_unaligned(ptr::addr_of!((*net_driver(this).cast::<UNetDriver>()).allow_downloads)) != 0 } }
    fn close_first_channel(&mut self, conn: *mut c_void) -> R<()> { unsafe { action!(self.0.channel_close, ptr::read_unaligned(ptr::addr_of!((*connection(conn)).channels[0]))); } Ok(()) }
    fn lone_player_slot(&mut self, this: *mut c_void) -> *mut i32 { unsafe { lone_player_slot(this) } }
    fn package_map(&mut self, conn: *mut c_void) -> *mut c_void { unsafe { package_map(conn) } }
    fn package_count(&mut self, map: *mut c_void) -> i32 { unsafe { package_list(map).num } }
    fn package_at(&mut self, map: *mut c_void, index: i32) -> *mut FPackageInfo { unsafe { package_info(map, index) } }
    fn package_flags(&mut self, info: *mut FPackageInfo) -> u32 { unsafe { ptr::read_unaligned(package_flags_slot(info)) } }
    fn request_file(&mut self, conn: *mut c_void, index: i32) -> R<()> { unsafe { action!(self.0.connection_receive_file, conn, index); } Ok(()) }
    fn delete_finished_download(&mut self, conn: *mut c_void) -> R<()> {
        unsafe {
            let download = ptr::read_unaligned(ptr::addr_of!((*connection(conn)).download));
            if !download.is_null() { action!(self.0.download_delete, download); }
        }
        Ok(())
    }
    fn set_success(&mut self, this: *mut c_void) { unsafe { ptr::write_unaligned(success_slot(this), 1); } }
    fn error_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw { unsafe { error_slot(this) } }
    fn fail_code_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw { unsafe { fail_code_slot(this) } }
    fn fail_url_slot(&mut self, this: *mut c_void) -> *mut TArrayRaw { unsafe { fail_url_slot(this) } }
    fn user_flags_slot(&mut self, conn: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*connection(conn)).user_flags) } }
    fn negotiated_version_slot(&mut self, conn: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*connection(conn)).negotiated_ver) } }
    fn challenge_slot(&mut self, conn: *mut c_void) -> *mut i32 { unsafe { ptr::addr_of_mut!((*connection(conn)).challenge) } }
    fn current_speed(&mut self, conn: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*connection(conn)).current_net_speed)) } }
    fn remote_version(&mut self, conn: *mut c_void) -> i32 { unsafe { ptr::read_unaligned(ptr::addr_of!((*connection(conn)).remote_version)) } }
    fn close_old_connection(&mut self, conn: *mut c_void) { unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*connection(conn)).state), USOCK_CLOSED); } }
    fn set_package_parent(&mut self, info: *mut FPackageInfo, parent: *mut c_void) { unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*info).parent), parent); } }
    fn set_package_flags(&mut self, info: *mut FPackageInfo, flags: u32) { unsafe { ptr::write_unaligned(package_flags_slot(info), flags); } }
    fn increment_files_needed(&mut self, this: *mut c_void) {
        unsafe { let needed = files_needed_slot(this); ptr::write_unaligned(needed, ptr::read_unaligned(needed).wrapping_add(1)); }
    }
    fn url_host_slot(&mut self, url: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*url.cast::<FURL>()).host) } }
    fn url_map_slot(&mut self, url: *mut c_void) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*url.cast::<FURL>()).map) } }
    fn server_connection(&mut self, this: *mut c_void) -> *mut c_void { unsafe { server_connection(net_driver(this)) } }
    fn download_class_name(&mut self, info: *mut FDownloadInfo) -> *const u32 { unsafe { fstring_text(ptr::addr_of!((*info).class_name)) } }
    fn set_download_class(&mut self, info: *mut FDownloadInfo, class: *mut c_void) { unsafe { ptr::write_unaligned(ptr::addr_of_mut!((*info).class), class); } }
    fn package_guid_slot(&mut self, info: *mut FPackageInfo) -> *mut [u32; 4] { unsafe { ptr::addr_of_mut!((*info).guid) } }
    fn package_generation_slot(&mut self, info: *mut FPackageInfo) -> *mut i32 { unsafe { ptr::addr_of_mut!((*info).remote_generation) } }
    fn package_size_slot(&mut self, info: *mut FPackageInfo) -> *mut i32 { unsafe { ptr::addr_of_mut!((*info).file_size) } }
    fn package_filename_slot(&mut self, info: *mut FPackageInfo) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*info).url) } }
    fn download_class_name_slot(&mut self, info: *mut FDownloadInfo) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*info).class_name) } }
    fn download_params_slot(&mut self, info: *mut FDownloadInfo) -> *mut TArrayRaw { unsafe { ptr::addr_of_mut!((*info).params) } }
    fn download_compression_slot(&mut self, info: *mut FDownloadInfo) -> *mut i32 { unsafe { ptr::addr_of_mut!((*info).compression) } }
    fn package_flags_slot(&mut self, info: *mut FPackageInfo) -> *mut u32 { unsafe { package_flags_slot(info) } }
}

/// `UNetPendingLevel::TrySkipFile`.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel` with a live driver and server connection.
pub unsafe fn try_skip_file(s: &Hp2NetPendingServices, this: *mut c_void) -> R<i32> {
    unsafe {
        let server = server_connection(net_driver(this));
        let download = ptr::read_unaligned(ptr::addr_of!((*connection(server)).download));
        if download.is_null() {
            return Ok(0);
        }
        Ok(i32::from(value!(s.download_try_skip_file, download) != 0))
    }
}

/// `UNetPendingLevel::NotifyReceivedFile`.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel`, `conn` a live `UNetConnection` with a
/// package map, and `error` NUL-terminated.
pub unsafe fn notify_received_file(
    s: &Hp2NetPendingServices,
    this: *mut c_void,
    conn: *mut c_void,
    package_index: i32,
    error: *const u32,
    skipped: i32,
) -> R<()> {
    unsafe { notify_received_file_with(&mut NativePendingFileOps(s), this, conn, package_index, error, skipped != 0) }
}

/// `UNetPendingLevel::ReceiveNextFile`: request the first needed package, or
/// delete the finished download.
///
/// # Safety
/// `conn` must be a live `UNetConnection` with a package map.
pub unsafe fn receive_next_file(s: &Hp2NetPendingServices, conn: *mut c_void) -> R<()> {
    unsafe { receive_next_file_with(&mut NativePendingFileOps(s), conn) }
}

/// `UNetPendingLevel::NotifySendingFile`: the server may not request client files.
///
/// # Safety
/// The services must be live.
pub unsafe fn notify_sending_file(s: &Hp2NetPendingServices) -> R<i32> {
    unsafe {
        let format = value!(s.localize_error, c"RequestDenied".as_ptr());
        action!(s.log_dev_net_format, format);
    }
    Ok(0)
}

/// Source-shared `UNetPendingLevel::Tick`/`SendJoin` leaves.
///
/// Canonical Core implements these directly; the native table forwards to the
/// original engine calls. Both paths keep the source order and the one-shot
/// `ConnectionFailed` assignment.
pub(crate) trait PendingTickOps {
    type Error;
    /// `appFailAssert(Expression, __FILE__, Line)` for a failed source check.
    fn check(
        &mut self,
        valid: bool,
        expression: &CStr,
        line: i32,
    ) -> Result<(), Self::Error>;
    /// `LocalizeError(key)`; the returned text stays live for the call.
    fn localized_error(&mut self, key: &CStr) -> Result<*const u32, Self::Error>;
    /// `*destination = text`.
    fn assign_fstring(
        &mut self,
        destination: *mut TArrayRaw,
        text: *const u32,
    ) -> Result<(), Self::Error>;
    /// `Driver->TickDispatch(DeltaSeconds)`.
    fn driver_tick(&mut self, driver: *mut c_void, delta: f32) -> Result<(), Self::Error>;
    /// `Driver->TickFlush()`.
    fn driver_flush(&mut self, driver: *mut c_void) -> Result<(), Self::Error>;
    /// `Connection->Logf(text)`.
    fn log_connection(
        &mut self,
        connection: *mut c_void,
        text: *const u32,
    ) -> Result<(), Self::Error>;
    /// `Connection->FlushNet()`.
    fn flush_connection(&mut self, connection: *mut c_void) -> Result<(), Self::Error>;
}

/// `UNetPendingLevel::Tick`: fail a closed connection, otherwise drive the driver.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel`.
pub(crate) unsafe fn tick_with<O: PendingTickOps>(
    ops: &mut O,
    this: *mut c_void,
    delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        let driver = net_driver(this);
        ops.check(!driver.is_null(), c"NetDriver", 312)?;
        let server = server_connection(driver);
        ops.check(!server.is_null(), c"NetDriver->ServerConnection", 313)?;

        // Handle a timed out or failed connection.
        let state = ptr::read_unaligned(ptr::addr_of!((*connection(server)).state));
        if state == USOCK_CLOSED && fstring_is_empty(error_slot(this)) {
            let text = ops.localized_error(c"ConnectionFailed")?;
            return ops.assign_fstring(error_slot(this), text);
        }

        ops.driver_tick(driver, delta)?;
        ops.driver_flush(driver)
    }
}

/// `UNetPendingLevel::SendJoin`.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel` with a live driver and server connection.
pub(crate) unsafe fn send_join_with<O: PendingTickOps>(
    ops: &mut O,
    this: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        ptr::write_unaligned(sent_join_slot(this), 1);
        let server = server_connection(net_driver(this));
        ops.log_connection(server, lit!("JOIN").as_ptr())?;
        ops.flush_connection(server)
    }
}

struct NativePendingTickOps<'a>(&'a Hp2NetPendingServices);

impl PendingTickOps for NativePendingTickOps<'_> {
    type Error = ();
    fn check(&mut self, valid: bool, expression: &CStr, line: i32) -> R<()> {
        check(self.0, expression, line, !valid)
    }
    fn localized_error(&mut self, key: &CStr) -> R<*const u32> {
        unsafe { Ok(value!(self.0.localize_error, key.as_ptr())) }
    }
    fn assign_fstring(&mut self, destination: *mut TArrayRaw, text: *const u32) -> R<()> {
        unsafe { action!(self.0.fstring_assign, destination.cast(), text); }
        Ok(())
    }
    fn driver_tick(&mut self, driver: *mut c_void, delta: f32) -> R<()> {
        unsafe { action!(self.0.driver_tick_dispatch, driver, delta); }
        Ok(())
    }
    fn driver_flush(&mut self, driver: *mut c_void) -> R<()> {
        unsafe { action!(self.0.driver_tick_flush, driver); }
        Ok(())
    }
    fn log_connection(&mut self, connection: *mut c_void, text: *const u32) -> R<()> {
        unsafe { action!(self.0.connection_log, connection, text); }
        Ok(())
    }
    fn flush_connection(&mut self, connection: *mut c_void) -> R<()> {
        unsafe { action!(self.0.connection_flush_net, connection); }
        Ok(())
    }
}

/// `UNetPendingLevel::Tick` through the native service table.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel` and `s` the initialized table.
pub unsafe fn tick(s: &Hp2NetPendingServices, this: *mut c_void, delta: f32) -> R<()> {
    unsafe { tick_with(&mut NativePendingTickOps(s), this, delta) }
}

/// `UNetPendingLevel::SendJoin` through the native service table.
///
/// # Safety
/// `this` must be a live `UNetPendingLevel` with a live driver and server connection.
pub unsafe fn send_join(s: &Hp2NetPendingServices, this: *mut c_void) -> R<()> {
    unsafe { send_join_with(&mut NativePendingTickOps(s), this) }
}

#[cfg(test)]
mod tests;

pub(crate) trait PendingFileOps {
    type Error;
    fn check(&mut self, valid: bool, expression: &CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn remove_download_method(&mut self, connection: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn record_failure(&mut self, destination: *mut TArrayRaw, parent: *mut c_void, error: *const u32) -> Result<(), Self::Error>;
    unsafe fn remove_package(&mut self, map: *mut c_void, index: i32) -> Result<(), Self::Error>;
    unsafe fn request_file(&mut self, connection: *mut c_void, index: i32) -> Result<(), Self::Error>;
    unsafe fn delete_download(&mut self, download: *mut c_void) -> Result<(), Self::Error>;
}

/// Runs the same package bookkeeping, retry-method and error-precedence policy
/// for native and canonical pending levels.
///
/// # Safety
/// Records, package slots and NUL-terminated error text remain live through calls.
pub(crate) unsafe fn notify_received_file_with<O: PendingFileOps>(
    ops: &mut O, this: *mut c_void, conn: *mut c_void, package_index: i32,
    error: *const u32, skipped: bool,
) -> Result<(), O::Error> {
    unsafe {
        let map = package_map(conn);
        ops.check(
            (0..package_list(map).num).contains(&package_index),
            c"Connection->PackageMap->List.IsValidIndex(PackageIndex)", 251,
        )?;
        let info = package_info(map, package_index);
        if *error != 0 {
            let infos = ptr::read_unaligned(ptr::addr_of!((*connection(conn)).download_info));
            if infos.num > 1 {
                ops.remove_download_method(conn)?;
                receive_next_file_with(ops, conn)?;
            } else if fstring_is_empty(error_slot(this)) {
                ops.record_failure(error_slot(this), package_parent(info), error)?;
            }
        } else {
            ops.check(
                conn == server_connection(net_driver(this)),
                c"Connection==NetDriver->ServerConnection", 273,
            )?;
            let flags = ptr::read_unaligned(package_flags_slot(info));
            ops.check(flags & PKG_NEED != 0, c"Info.PackageFlags&PKG_Need", 274)?;
            ptr::write_unaligned(package_flags_slot(info), flags & !PKG_NEED);
            let needed = files_needed_slot(this);
            ptr::write_unaligned(needed, ptr::read_unaligned(needed).wrapping_sub(1));
            if skipped { ops.remove_package(map, package_index)?; }
            receive_next_file_with(ops, conn)?;
        }
    }
    Ok(())
}

/// Requests the first needed package, or destroys the finished download.
///
/// # Safety
/// The connection and package map remain live through each request/destruction.
pub(crate) unsafe fn receive_next_file_with<O: PendingFileOps>(
    ops: &mut O, conn: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let mut index = 0;
        while index < package_list(package_map(conn)).num {
            if ptr::read_unaligned(package_flags_slot(package_info(package_map(conn), index))) & PKG_NEED != 0 {
                return ops.request_file(conn, index);
            }
            index += 1;
        }
        let download = ptr::read_unaligned(ptr::addr_of!((*connection(conn)).download));
        if !download.is_null() { ops.delete_download(download)?; }
    }
    Ok(())
}

struct NativePendingFileOps<'a>(&'a Hp2NetPendingServices);
impl PendingFileOps for NativePendingFileOps<'_> {
    type Error = ();
    fn check(&mut self, valid: bool, expression: &CStr, line: i32) -> R<()> {
        check(self.0, expression, line, !valid)
    }
    unsafe fn remove_download_method(&mut self, connection: *mut c_void) -> R<()> {
        unsafe { action!(self.0.download_info_remove, connection, 0); }
        Ok(())
    }
    unsafe fn record_failure(&mut self, destination: *mut TArrayRaw, parent: *mut c_void, error: *const u32) -> R<()> {
        unsafe {
            let format = value!(self.0.localize_error, c"DownloadFailed".as_ptr());
            let name = value!(self.0.object_name, parent);
            action!(self.0.fstring_printf_texts, destination.cast(), format, name, error);
        }
        Ok(())
    }
    unsafe fn remove_package(&mut self, map: *mut c_void, index: i32) -> R<()> {
        unsafe { action!(self.0.package_list_remove, map, index); }
        Ok(())
    }
    unsafe fn request_file(&mut self, connection: *mut c_void, index: i32) -> R<()> {
        unsafe { action!(self.0.connection_receive_file, connection, index); }
        Ok(())
    }
    unsafe fn delete_download(&mut self, download: *mut c_void) -> R<()> {
        unsafe { action!(self.0.download_delete, download); }
        Ok(())
    }
}
