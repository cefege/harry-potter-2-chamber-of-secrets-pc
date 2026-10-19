//! Rust port of `HarryPotter2/Unreal/Engine/Src/AStatLog.cpp` (pinned `69c3263`): the
//! `AStatLog` and `AStatLogFile` natives, `appMD5UpdateStringUtf16LE`, and
//! `UCheckSumCommandlet::Main`.
//!
//! Every engine object access goes through [`StatLogServices`], filled in by
//! `Hp2StatLogServices.cpp`. Those thunks are the C++ member reads, global lookups,
//! virtual calls, and engine free functions that the port cannot own. The
//! `IMPLEMENT_CLASS` lines, native tables, `P_GET_*`/`P_FINISH` unpacking, and
//! `guard`/`unguard` stay in the C++ shells.
//!
//! Errors: each native returns [`StatLogResult`]. The C++ shell maps an error to the
//! original `check`, `appErrorf`, or `appFailAssert` call. The Rust side never unwinds
//! through C++ frames.
//!
//! Divergences, all intentional:
//! - `Watermark` and `GetChecksum` without a live context: the original runs
//!   `check(Runtime && Runtime->Context)` and then dereferences `Runtime->Context`
//!   anyway. `check` ends the run through `appFailAssert`, so the dereference is never
//!   reached. The port returns [`StatLogError::CheckFailed`] before any dereference, and
//!   the shell calls `appFailAssert` with the same expression.
//! - Runtime state: the C++ `TArray<FStatLogRuntimeState>` becomes a Rust
//!   `Mutex<Vec<_>>` of (owner, context, archive) keys. The lock is never held across a
//!   service call, because `ProcessEvent` can re-enter `FileLog`.
//! - The static `LogBatcherHandle` becomes an [`AtomicUsize`]. It keeps the same
//!   process-wide lifetime.
//! - Overflow branches (`Bytes.Num() > MAXINT-2`) are kept, but they cannot be reached in
//!   practice. A 2 GiB buffer would fail first.
//! - UTF-16 surrogates: the original merges a high/low pair only when
//!   `sizeof(TCHAR) == sizeof(UNICHAR)`. Here `TCHAR` is 4-byte `wchar_t`
//!   (`HP2_HOST_WCHAR_TCHAR`), so that merge is dead code. The port keeps the same gate, so
//!   each surrogate unit becomes U+FFFD and nothing merges. Code points 0x10000..0x10FFFF
//!   are split into surrogate pairs.
//! - FMA: the object's 128 fused ops (112 `fmadd`, 16 `fmsub`) all sit in unreferenced
//!   `FCoords` operators from `UnMath.h`. No AStatLog native fuses. The port has no `mul_add`.
//! - Libm: none. The module makes no libm calls.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::params::TChar;

/// C++ services (`Hp2StatLogServices` in `Hp2StatLogServices.h`), same field order.
#[repr(C)]
pub struct StatLogServices {
    /// `AActor::Level` (ALevelInfo*).
    pub actor_level: unsafe extern "C" fn(actor: *mut c_void) -> *mut c_void,
    /// `ALevelInfo::Game` (AGameInfo*).
    pub level_game: unsafe extern "C" fn(level: *mut c_void) -> *mut c_void,
    /// `ALevelInfo::NetMode != NM_DedicatedServer` (nonzero means not dedicated).
    pub level_not_dedicated: unsafe extern "C" fn(level: *mut c_void) -> i32,
    /// `AGameInfo::bExternalBatcher`.
    pub game_external_batcher: unsafe extern "C" fn(game: *mut c_void) -> i32,
    /// `AGameInfo::LocalLogFileName` (FString*).
    pub game_local_log_file_name: unsafe extern "C" fn(game: *mut c_void) -> *mut c_void,
    /// `FString` data pointer (`**fstr`), valid for `fstr_len` units.
    pub fstr_chars: unsafe extern "C" fn(fstr: *mut c_void) -> *const TChar,
    /// `FString::Len()`.
    pub fstr_len: unsafe extern "C" fn(fstr: *mut c_void) -> i32,
    /// `FString = text` (NUL-terminated `text`).
    pub fstr_assign: unsafe extern "C" fn(fstr: *mut c_void, text: *const TChar),
    /// `FString += text` (NUL-terminated `text`).
    pub fstr_append: unsafe extern "C" fn(fstr: *mut c_void, text: *const TChar),
    /// `AStatLog::LocalBatcherURL` (FString*).
    pub stat_log_local_batcher_url: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `AStatLog::LocalLogDir` (FString*).
    pub stat_log_local_log_dir: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `AStatLog::WorldBatcherURL` (FString*).
    pub stat_log_world_batcher_url: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `AStatLog::WorldBatcherParams` (FString*).
    pub stat_log_world_batcher_params: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `GetClass()->GetDefaultObject()` (AStatLog*).
    pub stat_log_class_default_object: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `AStatLog::StaticClass()->GetDefaultObject()` (AStatLog*).
    pub stat_log_static_default_object: unsafe extern "C" fn() -> *mut c_void,
    /// `AStatLog::bWorldBatcherError`.
    pub stat_log_get_world_batcher_error: unsafe extern "C" fn(this: *mut c_void) -> i32,
    /// `AStatLog::bWorldBatcherError = value`.
    pub stat_log_set_world_batcher_error: unsafe extern "C" fn(this: *mut c_void, value: i32),
    /// `SaveConfig()`.
    pub stat_log_save_config: unsafe extern "C" fn(this: *mut c_void),
    /// `AStatLog::bWorld`.
    pub stat_log_get_world: unsafe extern "C" fn(this: *mut c_void) -> i32,
    /// `AStatLogFile::StatLogFile` (FString*).
    pub stat_log_file_name: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `AStatLogFile::StatLogFinal` (FString*).
    pub stat_log_file_final: unsafe extern "C" fn(this: *mut c_void) -> *mut c_void,
    /// `APlayerPawn::PlayerReplicationInfo->PlayerName` (FString*).
    pub stat_log_player_name: unsafe extern "C" fn(pawn: *mut c_void) -> *mut c_void,
    /// `APlayerPawn::ngWorldSecret` (FString*).
    pub stat_log_player_secret: unsafe extern "C" fn(pawn: *mut c_void) -> *mut c_void,
    /// `eventLogGameSpecial(id, target->GetClass()->GetFullName())`.
    pub stat_log_event_special_class:
        unsafe extern "C" fn(this: *mut c_void, id: *const TChar, target: *mut c_void),
    /// `eventLogGameSpecial2(id, param1, param2)`.
    pub stat_log_event_special2: unsafe extern "C" fn(
        this: *mut c_void,
        id: *const TChar,
        param1: *const TChar,
        param2: *const TChar,
    ),
    /// `TObjectIterator<UClass>`: calls `callback(user, CastChecked<UPackage>(class->GetOuter()))`
    /// once per class, in iterator order.
    pub class_iterate_packages: unsafe extern "C" fn(
        callback: unsafe extern "C" fn(user: *mut c_void, package: *mut c_void),
        user: *mut c_void,
    ),
    /// `UPackage::GetFullName()` (NUL-terminated, static buffer).
    pub package_full_name: unsafe extern "C" fn(package: *mut c_void) -> *const TChar,
    /// `DLLEXT` (NUL-terminated, static buffer).
    pub dll_ext: unsafe extern "C" fn() -> *const TChar,
    /// `GFileManager->FileSize(name)`.
    pub file_size: unsafe extern "C" fn(name: *const TChar) -> i32,
    /// `GFileManager->CreateFileWriter(name, FILEWRITE_EvenIfReadOnly)`.
    pub file_create_writer: unsafe extern "C" fn(name: *const TChar) -> *mut c_void,
    /// `GFileManager->MakeDirectory(name)`.
    pub file_make_directory: unsafe extern "C" fn(name: *const TChar),
    /// `GFileManager->Move(dst, src, 1, 1, 1)`.
    pub file_move: unsafe extern "C" fn(dst: *const TChar, src: *const TChar),
    /// `FArchive::Serialize(data, len)`.
    pub archive_serialize: unsafe extern "C" fn(ar: *mut c_void, data: *const u8, len: i32),
    /// `FArchive::Flush()`.
    pub archive_flush: unsafe extern "C" fn(ar: *mut c_void),
    /// `delete FArchive` (null is a no-op).
    pub archive_delete: unsafe extern "C" fn(ar: *mut c_void),
    /// `new FMD5Context` (heap).
    pub md5_context_new: unsafe extern "C" fn() -> *mut c_void,
    /// `appMD5Init(ctx)`.
    pub md5_context_init: unsafe extern "C" fn(ctx: *mut c_void),
    /// `appMD5Update(ctx, data, len)`.
    pub md5_context_update: unsafe extern "C" fn(ctx: *mut c_void, data: *const u8, len: i32),
    /// `appMD5Final(digest, ctx)`; `digest` has 16 bytes.
    pub md5_context_final: unsafe extern "C" fn(digest: *mut u8, ctx: *mut c_void),
    /// `delete FMD5Context` (null is a no-op).
    pub md5_context_delete: unsafe extern "C" fn(ctx: *mut c_void),
    /// `debugf(TEXT("%s"), text)`.
    pub debugf_text: unsafe extern "C" fn(text: *const TChar),
    /// `GWarn->Logf(TEXT("%s"), text)`.
    pub warn_text: unsafe extern "C" fn(text: *const TChar),
    /// `GIsRequestingExit = 1`.
    pub set_requesting_exit: unsafe extern "C" fn(),
    /// `appCreateProc(url, params, show)`.
    pub proc_create:
        unsafe extern "C" fn(url: *const TChar, params: *const TChar, show: i32) -> *mut c_void,
    /// `appGetProcReturnCode(handle, &rc)`; returns nonzero on success.
    pub proc_return_code: unsafe extern "C" fn(handle: *mut c_void, rc: *mut i32) -> i32,
    /// `appLaunchURL(GFileManager->GetDefaultDirectory() * url)`.
    pub launch_url: unsafe extern "C" fn(url: *const TChar),
}

/// Failure of a native. The C++ shell maps each variant to the original call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatLogError {
    /// `check(expr)` failed; the shell calls `appFailAssert(expr, __FILE__, __LINE__)`.
    CheckFailed(&'static str),
    /// `Could not open file: %s`; the shell calls `appErrorf`.
    FileMissing,
    /// `UTF-16LE text exceeds the 32-bit MD5 contract`; the shell calls `appErrorf`.
    Md5Overflow,
    /// `Stat log entry exceeds the 32-bit archive contract`; the shell calls `appErrorf`.
    ArchiveOverflow,
}

/// Result of a native. `Ok(())` is success.
pub type StatLogResult<T = ()> = Result<T, StatLogError>;

/// `MAXINT - 2`, the byte limit checked before each UTF-16 unit is appended.
const MAX_ENCODED_BYTES: usize = i32::MAX as usize - 2;

/// Secret key for `GetChecksum`, 16 bytes.
const CHECKSUM_SECRET: &[u8; 16] = b"M4yfGp69keJdDV1q";

/// Per-owner state of the original `FStatLogRuntimeState`.
#[derive(Clone, Copy)]
struct Runtime {
    owner: usize,
    /// `FMD5Context*` as an address, or 0.
    context: usize,
    /// `FArchive*` as an address, or 0.
    log_ar: usize,
}

static RUNTIME: Mutex<Vec<Runtime>> = Mutex::new(Vec::new());
static LOG_BATCHER_HANDLE: AtomicUsize = AtomicUsize::new(0);

fn runtime_table() -> MutexGuard<'static, Vec<Runtime>> {
    RUNTIME.lock().unwrap_or_else(PoisonError::into_inner)
}

fn runtime_find(owner: *mut c_void) -> Option<Runtime> {
    let owner = owner as usize;
    runtime_table().iter().copied().find(|r| r.owner == owner)
}

fn runtime_get_or_add(owner: *mut c_void) -> Runtime {
    let owner = owner as usize;
    let mut table = runtime_table();
    if let Some(r) = table.iter().copied().find(|r| r.owner == owner) {
        return r;
    }
    let r = Runtime {
        owner,
        context: 0,
        log_ar: 0,
    };
    table.push(r);
    r
}

fn runtime_update(owner: *mut c_void, update: impl FnOnce(&mut Runtime)) {
    let owner = owner as usize;
    if let Some(r) = runtime_table().iter_mut().find(|r| r.owner == owner) {
        update(r);
    }
}

fn runtime_remove(owner: *mut c_void) {
    let owner = owner as usize;
    runtime_table().retain(|r| r.owner != owner);
}

/// Wide text from ASCII or Unicode source text, one `TChar` per UTF-16 unit.
fn wide(text: &str) -> Vec<TChar> {
    text.encode_utf16().map(|u| u as TChar).collect()
}

/// NUL-terminated copy for the C++ thunks.
fn nul_terminated(text: &[TChar]) -> Vec<TChar> {
    let mut out = Vec::with_capacity(text.len() + 1);
    out.extend_from_slice(text);
    out.push(0);
    out
}

/// Copy of an `FString`'s units, read through the thunks.
///
/// # Safety
/// `fstr` must point to a live `FString`.
unsafe fn fstr_units(svc: &StatLogServices, fstr: *mut c_void) -> Vec<TChar> {
    let len = unsafe { (svc.fstr_len)(fstr) }.max(0) as usize;
    if len == 0 {
        return Vec::new();
    }
    let chars = unsafe { (svc.fstr_chars)(fstr) };
    unsafe { std::slice::from_raw_parts(chars, len) }.to_vec()
}

/// Copy of a NUL-terminated static buffer.
///
/// # Safety
/// `text` must point to a NUL-terminated `TChar` string.
unsafe fn read_nul(text: *const TChar) -> Vec<TChar> {
    let mut out = Vec::new();
    let mut p = text;
    loop {
        let unit = unsafe { *p };
        if unit == 0 {
            break;
        }
        out.push(unit);
        p = unsafe { p.add(1) };
    }
    out
}

/// Host `TCHAR` as a code point. Negative host values become U+FFFD.
fn host_code(unit: TChar) -> u32 {
    let raw = unit as i64;
    if raw >= 0 { raw as u32 } else { 0xfffd }
}

/// Appends one little-endian UTF-16 unit. Fails once `bytes.len() > MAXINT - 2`.
fn append_unit(bytes: &mut Vec<u8>, unit: u16) -> bool {
    if bytes.len() > MAX_ENCODED_BYTES {
        return false;
    }
    bytes.extend_from_slice(&unit.to_le_bytes());
    true
}

/// `EncodeStatLogUtf16LE`: host text to UTF-16LE with the original surrogate handling.
///
/// A high surrogate followed by a low surrogate combines into one code point. A high
/// surrogate without a following low surrogate, a lone low surrogate, and code points
/// above U+10FFFF each become U+FFFD. The following unit is not consumed in the first
/// case. Code points above U+FFFF become a surrogate pair. `append_line_ending` adds CR LF.
/// Returns `None` when the 32-bit byte limit is exceeded.
pub fn encode_utf16le(text: &[TChar], append_line_ending: bool) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len() * 2 + 2);
    let mut index = 0;
    while index < text.len() {
        let mut code = host_code(text[index]);
        if std::mem::size_of::<TChar>() == std::mem::size_of::<u16>()
            && (0xd800..=0xdbff).contains(&code)
        {
            let low = text.get(index + 1).map(|&u| host_code(u));
            match low {
                Some(lo) if (0xdc00..=0xdfff).contains(&lo) => {
                    code = 0x10000 + ((code - 0xd800) << 10) + (lo - 0xdc00);
                    index += 1;
                }
                _ => code = 0xfffd,
            }
        } else if (0xd800..=0xdfff).contains(&code) || code > 0x10ffff {
            code = 0xfffd;
        }
        if code > 0xffff {
            let v = code - 0x10000;
            if !append_unit(&mut bytes, (0xd800 + (v >> 10)) as u16)
                || !append_unit(&mut bytes, (0xdc00 + (v & 0x3ff)) as u16)
            {
                return None;
            }
        } else if !append_unit(&mut bytes, code as u16) {
            return None;
        }
        index += 1;
    }
    if append_line_ending && (!append_unit(&mut bytes, 0x0d) || !append_unit(&mut bytes, 0x0a)) {
        return None;
    }
    Some(bytes)
}

/// `appMD5UpdateStringUtf16LE`: encodes `text` and feeds it to the context.
///
/// # Safety
/// `ctx` must be a live `FMD5Context`.
pub unsafe fn md5_update_text(
    svc: &StatLogServices,
    ctx: *mut c_void,
    text: &[TChar],
    append_line_ending: bool,
) -> StatLogResult {
    let bytes = encode_utf16le(text, append_line_ending).ok_or(StatLogError::Md5Overflow)?;
    if !bytes.is_empty() {
        unsafe { (svc.md5_context_update)(ctx, bytes.as_ptr(), bytes.len() as i32) };
    }
    Ok(())
}

/// MD5 over the given texts with a fresh heap context. The context is always freed.
///
/// # Safety
/// The services must be live.
unsafe fn md5_digest(svc: &StatLogServices, parts: &[(&[TChar], bool)]) -> StatLogResult<[u8; 16]> {
    let ctx = unsafe { (svc.md5_context_new)() };
    unsafe { (svc.md5_context_init)(ctx) };
    let mut result: StatLogResult = Ok(());
    for (text, append_line_ending) in parts {
        result = unsafe { md5_update_text(svc, ctx, text, *append_line_ending) };
        if result.is_err() {
            break;
        }
    }
    let mut digest = [0u8; 16];
    if result.is_ok() {
        unsafe { (svc.md5_context_final)(digest.as_mut_ptr(), ctx) };
    }
    unsafe { (svc.md5_context_delete)(ctx) };
    result.map(|()| digest)
}

/// Sixteen `%02x` pairs as wide text.
fn hex_units(digest: &[u8; 16]) -> Vec<TChar> {
    let text: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    wide(&text)
}

/// `FString::Caps()`: ASCII lowercase a–z only.
fn caps_of(units: &[TChar]) -> Vec<TChar> {
    units
        .iter()
        .map(|&c| {
            let raw = c as i64;
            if (i64::from(b'a')..=i64::from(b'z')).contains(&raw) {
                (raw - 32) as TChar
            } else {
                c
            }
        })
        .collect()
}

/// `CheckString = CapsName + Printf("%i", FileSize)`.
fn check_string(caps: &[TChar], size: i32) -> Vec<TChar> {
    let mut out = caps.to_vec();
    out.extend(wide(&size.to_string()));
    out
}

/// `FileName.Right(FileName.Len() - (FileName.InStr(" ") + 1))`.
fn strip_through_space(name: &[TChar]) -> Vec<TChar> {
    match name.iter().position(|&c| c == wide(" ")[0]) {
        Some(i) => name[i + 1..].to_vec(),
        None => name.to_vec(),
    }
}

/// `FString::InStr(sub, Right=1)`: the index of the last occurrence.
fn last_index(units: &[TChar], ch: char) -> Option<usize> {
    let needle = ch as u32 as TChar;
    units.iter().rposition(|&c| c == needle)
}

/// Forwards `debugf(TEXT("%s"), text)`.
///
/// # Safety
/// The services must be live.
unsafe fn debugf(svc: &StatLogServices, text: &str) {
    let text = nul_terminated(&wide(text));
    unsafe { (svc.debugf_text)(text.as_ptr()) };
}

/// Creates a batcher process and returns its handle (null on failure).
///
/// # Safety
/// The services must be live.
unsafe fn create_proc(
    svc: &StatLogServices,
    url: &[TChar],
    params: &[TChar],
    show: i32,
) -> *mut c_void {
    let url = nul_terminated(url);
    let params = nul_terminated(params);
    unsafe { (svc.proc_create)(url.as_ptr(), params.as_ptr(), show) }
}

/// `AStatLog::CloseStatLogRuntimeState`: deletes the context and archive, then removes the entry.
///
/// # Safety
/// The services must be live.
unsafe fn close_runtime(svc: &StatLogServices, owner: *mut c_void) {
    let Some(r) = runtime_find(owner) else {
        return;
    };
    if r.context != 0 {
        unsafe { (svc.md5_context_delete)(r.context as *mut c_void) };
    }
    if r.log_ar != 0 {
        unsafe { (svc.archive_delete)(r.log_ar as *mut c_void) };
    }
    runtime_remove(owner);
}

/// `AStatLog::execExecuteLocalLogBatcher`.
///
/// # Safety
/// `this` must be a live `AStatLog`, and the services must be live.
pub unsafe fn execute_local_log_batcher(svc: &StatLogServices, this: *mut c_void) {
    let level = unsafe { (svc.actor_level)(this) };
    let game = unsafe { (svc.level_game)(level) };
    let url = unsafe { fstr_units(svc, (svc.stat_log_local_batcher_url)(this)) };
    let log_file_name = unsafe { fstr_units(svc, (svc.game_local_log_file_name)(game)) };
    let show = unsafe { (svc.level_not_dedicated)(level) };
    unsafe { create_proc(svc, &url, &log_file_name, show) };
}

/// `AStatLog::execExecuteSilentLogBatcher`.
///
/// # Safety
/// `this` must be a live `AStatLog`, and the services must be live.
pub unsafe fn execute_silent_log_batcher(svc: &StatLogServices, this: *mut c_void) {
    let level = unsafe { (svc.actor_level)(this) };
    let game = unsafe { (svc.level_game)(level) };
    let url = unsafe { fstr_units(svc, (svc.stat_log_local_batcher_url)(this)) };
    let log_file_name = unsafe { fstr_units(svc, (svc.game_local_log_file_name)(game)) };
    let mut params = wide("-b false ");
    params.extend_from_slice(&log_file_name);
    let show = unsafe { (svc.level_not_dedicated)(level) };
    unsafe { create_proc(svc, &url, &params, show) };
}

/// `AStatLog::execBatchLocal`: reads the URL and directory from the class default object.
///
/// # Safety
/// `this` must be a live `AStatLog`, and the services must be live.
pub unsafe fn batch_local(svc: &StatLogServices, this: *mut c_void) {
    let defaults = unsafe { (svc.stat_log_class_default_object)(this) };
    let url = unsafe { fstr_units(svc, (svc.stat_log_local_batcher_url)(defaults)) };
    let dir = unsafe { fstr_units(svc, (svc.stat_log_local_log_dir)(defaults)) };
    unsafe { create_proc(svc, &url, &dir, 1) };
}

/// `AStatLog::execBrowseRelativeLocalURL`.
///
/// # Safety
/// `url` must be a live `FString`, and the services must be live.
pub unsafe fn browse_relative_local_url(svc: &StatLogServices, url: *mut c_void) {
    let url_units = unsafe { fstr_units(svc, url) };
    let url = nul_terminated(&url_units);
    unsafe { (svc.launch_url)(url.as_ptr()) };
}

/// `AStatLog::execExecuteWorldLogBatcher`.
///
/// # Safety
/// `this` must be a live `AStatLog`, and the services must be live.
pub unsafe fn execute_world_log_batcher(svc: &StatLogServices, this: *mut c_void) -> StatLogResult {
    let level = unsafe { (svc.actor_level)(this) };
    let game = unsafe { (svc.level_game)(level) };
    if game.is_null() {
        return Err(StatLogError::CheckFailed("Level->Game"));
    }
    if unsafe { (svc.game_external_batcher)(game) } != 0 {
        return Ok(());
    }
    unsafe { debugf(svc, "ngWorldStats: ExecuteWorldLogBatcher") };
    let handle = LOG_BATCHER_HANDLE.load(Ordering::Relaxed) as *mut c_void;
    if !handle.is_null() {
        let mut return_code: i32 = 0;
        if unsafe { (svc.proc_return_code)(handle, &mut return_code) } == 0 {
            // The batcher is still running, or the handle is invalid. A new copy replaces it.
            unsafe {
                debugf(
                    svc,
                    "ngWorldStats: Old batcher was still running or handle was invalid.",
                )
            };
        } else if return_code >= 0 {
            unsafe { (svc.stat_log_set_world_batcher_error)(this, 0) };
        } else {
            unsafe {
                debugf(
                    svc,
                    &format!(
                        "ngWorldStats: Previous batcher failed with return code {return_code}."
                    ),
                )
            };
            unsafe { (svc.stat_log_set_world_batcher_error)(this, 1) };
        }
    }
    let url = unsafe { fstr_units(svc, (svc.stat_log_world_batcher_url)(this)) };
    let params = unsafe { fstr_units(svc, (svc.stat_log_world_batcher_params)(this)) };
    let show = unsafe { (svc.level_not_dedicated)(level) };
    let new_handle = unsafe { create_proc(svc, &url, &params, show) };
    LOG_BATCHER_HANDLE.store(new_handle as usize, Ordering::Relaxed);
    if new_handle.is_null() {
        unsafe { debugf(svc, "ngWorldStats: Failed to launch batcher.") };
        unsafe { (svc.stat_log_set_world_batcher_error)(this, 1) };
    }
    let error = unsafe { (svc.stat_log_get_world_batcher_error)(this) };
    unsafe { debugf(svc, &format!("bWorldBatchError is {error}")) };
    let defaults = unsafe { (svc.stat_log_static_default_object)() };
    unsafe { (svc.stat_log_set_world_batcher_error)(defaults, error) };
    unsafe { (svc.stat_log_save_config)(this) };
    Ok(())
}

/// `AStatLog::eventLogGameSpecial` with the class name of `target`.
///
/// # Safety
/// The services and objects must be live.
unsafe fn event_special_class(
    svc: &StatLogServices,
    this: *mut c_void,
    id: &str,
    target: *mut c_void,
) {
    let id = nul_terminated(&wide(id));
    unsafe { (svc.stat_log_event_special_class)(this, id.as_ptr(), target) };
}

/// `AStatLog::eventLogGameSpecial2` with the `CodePackageChecksum` ID.
///
/// # Safety
/// The services and objects must be live.
unsafe fn event_checksum(
    svc: &StatLogServices,
    this: *mut c_void,
    file_name: &[TChar],
    checksum: &[TChar],
) {
    let id = nul_terminated(&wide("CodePackageChecksum"));
    let file_name = nul_terminated(file_name);
    let checksum = nul_terminated(checksum);
    unsafe {
        (svc.stat_log_event_special2)(this, id.as_ptr(), file_name.as_ptr(), checksum.as_ptr())
    };
}

/// Package-iteration callback. Appends each package once, in first-seen order.
///
/// # Safety
/// `user` must point to a live `Vec<*mut c_void>`.
unsafe extern "C" fn collect_package(user: *mut c_void, package: *mut c_void) {
    let packages = unsafe { &mut *user.cast::<Vec<*mut c_void>>() };
    if !packages.contains(&package) {
        packages.push(package);
    }
}

/// `AStatLog::execInitialCheck`: logs the game class, then each code package's checksum.
///
/// # Safety
/// `this` must be a live `AStatLog`, `game` a live `AGameInfo`, and the services live.
pub unsafe fn initial_check(
    svc: &StatLogServices,
    this: *mut c_void,
    game: *mut c_void,
) -> StatLogResult {
    unsafe { event_special_class(svc, this, "GameClass", game) };

    let mut packages: Vec<*mut c_void> = Vec::new();
    unsafe {
        (svc.class_iterate_packages)(
            collect_package,
            (&mut packages as *mut Vec<*mut c_void>).cast(),
        )
    };

    for package in packages {
        // CapsName is declared inside the loop, so it resets per package. The DLL pass
        // below appends to it, so the .u and DLL entries share one CapsName.
        let mut caps: Vec<TChar> = Vec::new();

        let full_name = unsafe { read_nul((svc.package_full_name)(package)) };
        let mut u_name = full_name.clone();
        u_name.extend(wide(".u"));
        let u_name = strip_through_space(&u_name);
        let u_size = unsafe { (svc.file_size)(nul_terminated(&u_name).as_ptr()) };
        caps.extend(caps_of(&u_name));
        if u_size != -1 {
            let check = check_string(&caps, u_size);
            let digest = unsafe { md5_digest(svc, &[(&check, false)])? };
            unsafe { event_checksum(svc, this, &u_name, &hex_units(&digest)) };
        }

        let dll_ext = unsafe { read_nul((svc.dll_ext)()) };
        let mut dll_name = full_name;
        dll_name.extend(dll_ext);
        let dll_name = strip_through_space(&dll_name);
        let dll_size = unsafe { (svc.file_size)(nul_terminated(&dll_name).as_ptr()) };
        caps.extend(caps_of(&dll_name));
        if dll_size != -1 {
            let check = check_string(&caps, dll_size);
            let digest = unsafe { md5_digest(svc, &[(&check, false)])? };
            unsafe { event_checksum(svc, this, &dll_name, &hex_units(&digest)) };
        }
    }
    Ok(())
}

/// `AStatLog::execLogMutator`.
///
/// # Safety
/// `this` must be a live `AStatLog`, `mutator` a live `AMutator`, and the services live.
pub unsafe fn log_mutator(svc: &StatLogServices, this: *mut c_void, mutator: *mut c_void) {
    unsafe { event_special_class(svc, this, "GameMutator", mutator) };
}

/// `AStatLog::execGetPlayerChecksum`.
///
/// # Safety
/// `pawn` must be a live `APlayerPawn`, `checksum` a live `FString`, and the services live.
pub unsafe fn get_player_checksum(
    svc: &StatLogServices,
    pawn: *mut c_void,
    checksum: *mut c_void,
) -> StatLogResult {
    let secret = unsafe { fstr_units(svc, (svc.stat_log_player_secret)(pawn)) };
    if secret.is_empty() {
        let text = nul_terminated(&wide("NoChecksum"));
        unsafe { (svc.fstr_assign)(checksum, text.as_ptr()) };
        return Ok(());
    }
    let name = unsafe { fstr_units(svc, (svc.stat_log_player_name)(pawn)) };
    let digest = unsafe { md5_digest(svc, &[(&name, false), (&secret, false)])? };
    let text = nul_terminated(&hex_units(&digest));
    unsafe { (svc.fstr_assign)(checksum, text.as_ptr()) };
    Ok(())
}

/// `AStatLogFile::execOpenLog`.
///
/// # Safety
/// `this` must be a live `AStatLogFile`, and the services must be live. `log_dir` is the
/// NUL-terminated `TEXT("..") PATH_SEPARATOR TEXT("Logs")` built by the C++ shell, so the
/// separator matches the target's `PATH_SEPARATOR`.
pub unsafe fn open_log(svc: &StatLogServices, this: *mut c_void, log_dir: *const TChar) {
    unsafe { close_runtime(svc, this) };
    runtime_get_or_add(this);
    let dir = nul_terminated(&unsafe { read_nul(log_dir) });
    unsafe { (svc.file_make_directory)(dir.as_ptr()) };
    let file_name = unsafe { fstr_units(svc, (svc.stat_log_file_name)(this)) };
    let file_name = nul_terminated(&file_name);
    let log_ar = unsafe { (svc.file_create_writer)(file_name.as_ptr()) };
    runtime_update(this, |r| r.log_ar = log_ar as usize);
    if unsafe { (svc.stat_log_get_world)(this) } != 0 {
        let ctx = unsafe { (svc.md5_context_new)() };
        unsafe { (svc.md5_context_init)(ctx) };
        runtime_update(this, |r| r.context = ctx as usize);
    }
}

/// `AStatLogFile::execCloseLog`.
///
/// # Safety
/// `this` must be a live `AStatLogFile`, and the services must be live.
pub unsafe fn close_log(svc: &StatLogServices, this: *mut c_void) {
    unsafe { close_runtime(svc, this) };
    let final_name = unsafe { fstr_units(svc, (svc.stat_log_file_final)(this)) };
    let file_name = unsafe { fstr_units(svc, (svc.stat_log_file_name)(this)) };
    let final_name = nul_terminated(&final_name);
    let file_name = nul_terminated(&file_name);
    unsafe { (svc.file_move)(final_name.as_ptr(), file_name.as_ptr()) };
}

/// `AStatLogFile::execWatermark`.
///
/// # Safety
/// `this` must be a live `AStatLogFile`, `event` a live `FString`, and the services live.
pub unsafe fn watermark(
    svc: &StatLogServices,
    this: *mut c_void,
    event: *mut c_void,
) -> StatLogResult {
    let context = match runtime_find(this) {
        Some(r) if r.context != 0 => r.context as *mut c_void,
        _ => return Err(StatLogError::CheckFailed("Runtime && Runtime->Context")),
    };
    let text = unsafe { fstr_units(svc, event) };
    unsafe { md5_update_text(svc, context, &text, true) }
}

/// `AStatLogFile::execGetChecksum`. Appends the digest to `checksum`. The context stays alive.
///
/// # Safety
/// `this` must be a live `AStatLogFile`, `checksum` a live `FString`, and the services live.
pub unsafe fn get_checksum(
    svc: &StatLogServices,
    this: *mut c_void,
    checksum: *mut c_void,
) -> StatLogResult {
    let context = match runtime_find(this) {
        Some(r) if r.context != 0 => r.context as *mut c_void,
        _ => return Err(StatLogError::CheckFailed("Runtime && Runtime->Context")),
    };
    unsafe { (svc.md5_context_update)(context, CHECKSUM_SECRET.as_ptr(), 16) };
    let mut digest = [0u8; 16];
    unsafe { (svc.md5_context_final)(digest.as_mut_ptr(), context) };
    let text = nul_terminated(&hex_units(&digest));
    unsafe { (svc.fstr_append)(checksum, text.as_ptr()) };
    Ok(())
}

/// `AStatLogFile::execFileFlush`.
///
/// # Safety
/// `this` must be a live `AStatLogFile`, and the services must be live.
pub unsafe fn file_flush(svc: &StatLogServices, this: *mut c_void) {
    if let Some(r) = runtime_find(this)
        && r.log_ar != 0
    {
        unsafe { (svc.archive_flush)(r.log_ar as *mut c_void) };
    }
}

/// `AStatLogFile::execFileLog`. Encodes the entry, XORs with `0xa7` for world logs, and writes.
///
/// # Safety
/// `this` must be a live `AStatLogFile`, `event` a live `FString`, and the services live.
pub unsafe fn file_log(
    svc: &StatLogServices,
    this: *mut c_void,
    event: *mut c_void,
) -> StatLogResult {
    let log_ar = runtime_find(this).map_or(0, |r| r.log_ar);
    let text = unsafe { fstr_units(svc, event) };
    let mut bytes = encode_utf16le(&text, true).ok_or(StatLogError::ArchiveOverflow)?;
    if unsafe { (svc.stat_log_get_world)(this) } != 0 {
        for b in &mut bytes {
            *b ^= 0xa7;
        }
    }
    if log_ar != 0 {
        unsafe {
            (svc.archive_serialize)(log_ar as *mut c_void, bytes.as_ptr(), bytes.len() as i32)
        };
    }
    Ok(())
}

/// `UCheckSumCommandlet::Main`.
///
/// # Safety
/// `file_name` must be a NUL-terminated `TCHAR` string, and the services must be live.
pub unsafe fn checksum_commandlet_main(
    svc: &StatLogServices,
    file_name: *const TChar,
) -> StatLogResult {
    let size = unsafe { (svc.file_size)(file_name) };
    if size < 0 {
        return Err(StatLogError::FileMissing);
    }
    let name = unsafe { read_nul(file_name) };
    let mut caps = caps_of(&name);
    if let Some(i) = last_index(&caps, '\\') {
        caps = caps[i..].to_vec();
    }
    if let Some(i) = last_index(&caps, '/') {
        caps = caps[i..].to_vec();
    }
    let check = check_string(&caps, size);
    let digest = unsafe { md5_digest(svc, &[(&check, false)])? };
    let checksum = hex_units(&digest);

    let mut message = wide("Package ");
    message.extend_from_slice(&name);
    message.extend(wide(" has checksum "));
    message.extend_from_slice(&checksum);
    let message = nul_terminated(&message);
    unsafe { (svc.warn_text)(message.as_ptr()) };
    unsafe { (svc.set_requesting_exit)() };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_units_is_lowercase_pairs() {
        let digest: [u8; 16] = [
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
            0x42, 0x7e,
        ];
        let text: String = hex_units(&digest)
            .iter()
            .map(|&u| char::from(u as u8))
            .collect();
        assert_eq!(text, "d41d8cd98f00b204e9800998ecf8427e");
    }

    /// The 2-byte surrogate-merge gate is off for 4-byte `TCHAR` (non-Windows targets).
    #[cfg(not(windows))]
    #[test]
    fn encode_without_merge_on_wide_tchar() {
        let u = |c: i32| c as TChar;
        assert_eq!(encode_utf16le(&[], false), Some(Vec::new()));
        assert_eq!(
            encode_utf16le(&[u(0x41)], true),
            Some(vec![0x41, 0, 0x0d, 0, 0x0a, 0])
        );
        assert_eq!(
            encode_utf16le(&[u(0xd800), u(0xdc00)], false),
            Some(vec![0xfd, 0xff, 0xfd, 0xff])
        );
        assert_eq!(encode_utf16le(&[u(-1)], false), Some(vec![0xfd, 0xff]));
        assert_eq!(
            encode_utf16le(&[u(0x110000)], false),
            Some(vec![0xfd, 0xff])
        );
        assert_eq!(
            encode_utf16le(&[u(0x10000)], false),
            Some(vec![0x00, 0xd8, 0x00, 0xdc])
        );
        assert_eq!(
            encode_utf16le(&[u(0x10ffff)], false),
            Some(vec![0xff, 0xdb, 0xff, 0xdf])
        );
    }
}
