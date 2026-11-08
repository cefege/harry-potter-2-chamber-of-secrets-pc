//! Canonical Rust ownership for Engine's `FReplay` state and replay files.

use hp2_abi::engine::generated::{FReplay, FReplayInputEvent, TArrayRaw};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::PathBuf;
use std::ptr;

const ARCHIVE_TAG: usize = 5;
const ARCHIVE_TAG_MASK: usize = 7;

/// Rust-owned archive state stored in the native opaque `file_ar` slot.
struct ReplayArchive {
    file: ReplayFile,
    consumed_frames: u64,
    first_tick_bits: Option<u32>,
    url: String,
    seeded: bool,
    completion_reported: bool,
}

enum ReplayFile {
    Reader(BufReader<File>),
    Writer(BufWriter<File>),
}
impl ReplayFile {
    fn reader(&mut self) -> RuntimeResult<&mut BufReader<File>> {
        match self { Self::Reader(file) => Ok(file), Self::Writer(_) => Err(RuntimeError::new("Replay archive is not readable")) }
    }
    fn writer(&mut self) -> RuntimeResult<&mut BufWriter<File>> {
        match self { Self::Writer(file) => Ok(file), Self::Reader(_) => Err(RuntimeError::new("Replay archive is not writable")) }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self { Self::Writer(file) => file.flush(), Self::Reader(_) => Ok(()) }
    }
    fn at_end(&mut self) -> RuntimeResult<bool> {
        let reader = self.reader()?;
        let length = reader.get_ref().metadata()
            .map_err(|error| RuntimeError::new("Replay metadata failed").with_context(&error.to_string()))?.len();
        let position = reader.stream_position()
            .map_err(|error| RuntimeError::new("Replay position failed").with_context(&error.to_string()))?;
        Ok(position >= length)
    }
}

unsafe fn archive_from_handle<'a>(raw: *mut c_void) -> Option<&'a mut ReplayArchive> {
    if raw.is_null() || raw.addr() & ARCHIVE_TAG_MASK != ARCHIVE_TAG { return None; }
    // SAFETY: only the tagged handle created by this module is dereferenced.
    Some(unsafe { &mut *raw.map_addr(|address| address & !ARCHIVE_TAG_MASK).cast::<ReplayArchive>() })
}


unsafe fn delete_archive(raw: *mut c_void) -> RuntimeResult<()> {
    if raw.is_null() { return Ok(()); }
    let archive = unsafe { archive_from_handle(raw) }
        .ok_or_else(|| RuntimeError::new("Invalid FReplay archive tag"))?;
    archive.file.flush()
        .map_err(|error| RuntimeError::new("Replay file flush failed").with_context(&error.to_string()))?;
    // SAFETY: raw is the tagged Box allocation installed in FReplay::file_ar.
    unsafe { drop(Box::from_raw(raw.map_addr(|address| address & !ARCHIVE_TAG_MASK).cast::<ReplayArchive>())); }
    Ok(())
}

/// # Safety
/// The live FString remains unchanged for the lifetime of the returned view.
unsafe fn string_slice<'a>(value: *const TArrayRaw) -> RuntimeResult<&'a [u32]> {
    let units = unsafe { fstring_units(value)? };
    // SAFETY: canonical FString units and TCHAR are both four-byte native units;
    // this is a bit-preserving view, not a Unicode conversion or an allocation.
    Ok(unsafe { std::slice::from_raw_parts(units.as_ptr().cast::<u32>(), units.len()) })
}

struct OwnerOps<'a>(CoreAccess<'a>);
impl super::net_replay::ReplayOps for OwnerOps<'_> {
    type Error = RuntimeError;

    unsafe fn log_already_recording(&self, replay: *mut FReplay, _log: *mut c_void) -> RuntimeResult<()> {
        unsafe { log_file_name(self.0, ptr::addr_of!((*replay).file_name), "Already recording: ") }
    }
    unsafe fn log_already_replaying(&self, replay: *mut FReplay, _log: *mut c_void) -> RuntimeResult<()> {
        unsafe { log_file_name(self.0, ptr::addr_of!((*replay).file_name), "Already replaying: ") }
    }
    unsafe fn assign_file_name(&self, replay: *mut FReplay, input: *const c_void) -> RuntimeResult<()> {
        if input.is_null() { return Err(RuntimeError::new("Null replay filename")); }
        let name = unsafe { terminated_units(input) };
        let path = replay_path(self.0, name)?;
        let units = path_to_units(&path);
        assign(self.0, unsafe { ptr::addr_of_mut!((*replay).file_name) }, &units)
    }
    unsafe fn create_file(&self, replay: *mut FReplay, record: bool) -> RuntimeResult<*mut c_void> {
        let path = replay_file_path(unsafe { ptr::addr_of!((*replay).file_name) })?;
        let opened = if record {
            OpenOptions::new().write(true).create(true).truncate(true).open(path)
        } else {
            OpenOptions::new().read(true).open(path)
        };
        let Ok(file) = opened else { return Ok(ptr::null_mut()); };
        let file = if record { ReplayFile::Writer(BufWriter::new(file)) } else { ReplayFile::Reader(BufReader::new(file)) };
        let base = Box::into_raw(Box::new(ReplayArchive {
            file, consumed_frames: 0, first_tick_bits: None, url: String::new(),
            seeded: false, completion_reported: false,
        }));
        debug_assert_eq!(base.addr() & ARCHIVE_TAG_MASK, 0);
        Ok(base.map_addr(|address| address | ARCHIVE_TAG).cast::<c_void>())
    }
    unsafe fn assign_url(&self, replay: *mut FReplay, start: *const c_void) -> RuntimeResult<()> {
        if start.is_null() {
            let url = unsafe { string_slice(ptr::addr_of!((*replay).url_str))? };
            if !url.is_empty() {
                return Err(RuntimeError::new("Null replay start URL with non-empty FString"));
            }
            return assign(self.0, unsafe { ptr::addr_of_mut!((*replay).url_str) }, &[]);
        }
        let text = unsafe { terminated_units(start) };
        assign(self.0, unsafe { ptr::addr_of_mut!((*replay).url_str) }, text)
    }
    unsafe fn serialize_url(&self, raw: *mut c_void, replay: *mut FReplay) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw) }.ok_or_else(|| RuntimeError::new("Invalid FReplay archive tag"))?;
        if unsafe { super::net_replay::recording(replay) } {
            let text = unsafe { string_slice(ptr::addr_of!((*replay).url_str))? };
            fstring_to_wire(text, archive.file.writer()?)
        } else {
            let text = wire_to_fstring(archive.file.reader()?)?;
            assign(self.0, unsafe { ptr::addr_of_mut!((*replay).url_str) }, &text)
        }
    }
    unsafe fn log_cannot_open(&self, replay: *mut FReplay, _log: *mut c_void, record: bool) -> RuntimeResult<()> {
        let prefix = if record { "Cannot create file " } else { "Cannot open file " };
        unsafe { log_file_name(self.0, ptr::addr_of!((*replay).file_name), prefix) }
    }
    unsafe fn delete(&self, archive: *mut c_void) -> RuntimeResult<()> { unsafe { delete_archive(archive) } }
    unsafe fn at_end(&self, raw: *mut c_void) -> RuntimeResult<bool> {
        let archive = unsafe { archive_from_handle(raw) }.ok_or_else(|| RuntimeError::new("Invalid FReplay archive tag"))?;
        let ended = archive.file.at_end()?;
        if ended && archive.seeded && !archive.completion_reported {
            if let Some(first_tick_bits) = archive.first_tick_bits {
                let message = format!(
                    "<HP2_REPLAY> url={} frames={} first_tick_bits={first_tick_bits:08x} seed=1 status=consumed",
                    archive.url, archive.consumed_frames,
                );
                let category = self.0.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
                let message: Vec<u32> = message.chars().map(u32::from).collect();
                self.0.emit_log(category, &message)?;
                archive.completion_reported = true;
            }
        }
        Ok(ended)
    }
    unsafe fn serialize_input(&self, raw: *mut c_void, event: *mut FReplayInputEvent) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw) }.ok_or_else(|| RuntimeError::new("Invalid FReplay archive tag"))?;
        match &mut archive.file {
            ReplayFile::Writer(file) => write_input(file, unsafe { *event }),
            ReplayFile::Reader(file) => {
                read_input(file, event)?;
                if unsafe { (*event).i_key == super::net_replay::IK_PLAY
                    && (*event).state == super::net_replay::IST_AXIS }
                {
                    archive.consumed_frames += 1;
                    archive.first_tick_bits.get_or_insert(unsafe { (*event).delta.to_bits() });
                }
                Ok(())
            }
        }
    }
    unsafe fn flush(&self, raw: *mut c_void) -> RuntimeResult<()> {
        let archive = unsafe { archive_from_handle(raw) }.ok_or_else(|| RuntimeError::new("Invalid FReplay archive tag"))?;
        archive.file.flush().map_err(|error| RuntimeError::new("Replay file flush failed").with_context(&error.to_string()))
    }
    unsafe fn reseed(&self) -> RuntimeResult<()> {
        hp2_core::rng_private::srand(1);
        hp2_core::ansi_trace::with_process_state(hp2_core::ansi_trace::reset_for_replay);
        Ok(())
    }
}

/// Borrows a NUL-terminated TCHAR slice, retaining its terminator.
///
/// # Safety
/// `input` must be non-null, aligned, readable through its NUL, and live for the returned borrow.
unsafe fn terminated_units<'a>(input: *const c_void) -> &'a [u32] {
    let input = input.cast::<u32>();
    let mut length = 0;
    while unsafe { *input.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: the caller supplies a readable NUL-terminated TCHAR input.
    unsafe { std::slice::from_raw_parts(input, length + 1) }
}

/// Borrows the characters of a validated Core-owned FString.
///
/// # Safety
/// `value` must address a live Core-owned FString that remains unchanged for the returned borrow.
unsafe fn fstring_units<'a>(value: *const TArrayRaw) -> RuntimeResult<&'a [hp2_core::ansi::Tchar]> {
    let array = unsafe { value.read_unaligned() };
    if array.num < 0 || (array.num > 0 && array.data.is_null()) {
        return Err(RuntimeError::new("Invalid replay FString storage"));
    }
    // SAFETY: the caller guarantees a live Core-owned FString for this borrow.
    Ok(unsafe { hp2_core::native_text::fstring_units(value) })
}

/// Emits one replay filename log message without copying its FString through an owned string.
///
/// # Safety
/// `file_name` must point to an initialized Core-owned FString that remains live during the call.
unsafe fn log_file_name(
    access: CoreAccess<'_>,
    file_name: *const TArrayRaw,
    prefix: &str,
) -> RuntimeResult<()> {
    let name = unsafe { fstring_units(file_name)? };
    let category = access.intern_str("Log", hp2_core::name_owner::FindName::Add)?;
    let mut message = Vec::with_capacity(prefix.len() + name.len());
    message.extend(prefix.bytes().map(u32::from));
    message.extend(name.iter().filter_map(|&unit| char::from_u32(unit as u32).map(u32::from)));
    access.emit_log(category, &message)
}

fn replay_file_path(file_name: *const TArrayRaw) -> RuntimeResult<PathBuf> {
    let name = unsafe { fstring_units(file_name)? };
    let path: String = name.iter().filter_map(|&unit| char::from_u32(unit as u32)).collect();
    Ok(PathBuf::from(path))
}

fn assign(access: CoreAccess<'_>, string: *mut TArrayRaw, value: &[u32]) -> RuntimeResult<()> {
    let nul = value.iter().position(|&unit| unit == 0).unwrap_or(value.len());
    if nul < value.len() {
        // SAFETY: string is an initialized FString; this slice includes its first NUL.
        return unsafe { access.assign_fstring(string, &value[..=nul]) };
    }
    let mut terminated = Vec::with_capacity(value.len() + 1);
    terminated.extend_from_slice(value);
    terminated.push(0);
    // SAFETY: string is an initialized FString in a live FReplay.
    unsafe { access.assign_fstring(string, &terminated) }
}
fn fstring_to_wire(text: &[u32], file: &mut impl Write) -> RuntimeResult<()> {
    use hp2_core::fstring_archive::{self, FStringWireUnit};
    let length = fstring_archive::utf32_wire_length(text)
        .ok_or_else(|| RuntimeError::new("Replay URL is too long"))?;
    hp2_core::compact_index::encode(length, |byte| {
        write_all(file, &[byte])?;
        Ok::<u8, RuntimeError>(byte)
    })?;
    fstring_archive::encode_utf32(text, length > 0, |kind, unit| {
        match kind {
            FStringWireUnit::Ansi => write_all(file, &[unit as u8]),
            FStringWireUnit::Utf16 => write_all(file, &unit.to_le_bytes()),
        }
    })
}

fn wire_to_fstring(file: &mut impl Read) -> RuntimeResult<Vec<u32>> {
    use hp2_core::fstring_archive::{self, FStringWireUnit};
    let length = hp2_core::compact_index::decode(|| {
        let mut byte = [0];
        read_exact(file, &mut byte)?;
        Ok::<u8, RuntimeError>(byte[0])
    })?;
    fstring_archive::decode_utf32(length, |kind| {
        let unit = match kind {
            FStringWireUnit::Ansi => {
                let mut byte = [0];
                read_exact(file, &mut byte)?;
                u16::from(byte[0])
            }
            FStringWireUnit::Utf16 => {
                let mut pair = [0; 2];
                read_exact(file, &mut pair)?;
                u16::from_le_bytes(pair)
            }
        };
        Ok::<u16, RuntimeError>(unit)
    })
}

fn write_all(file: &mut impl Write, bytes: &[u8]) -> RuntimeResult<()> {
    file.write_all(bytes).map_err(|error| RuntimeError::new("Replay file write failed").with_context(&error.to_string()))
}
fn read_exact(file: &mut impl Read, bytes: &mut [u8]) -> RuntimeResult<()> {
    file.read_exact(bytes).map_err(|error| RuntimeError::new("Replay file read failed").with_context(&error.to_string()))
}

fn replay_path(_access: CoreAccess<'_>, name: &[u32]) -> RuntimeResult<PathBuf> {
    let mut full = String::new();
    let mut cursor = hp2_core::unix_identity::user_dir();
    if cursor.is_null() { return Err(RuntimeError::new("Replay user directory is unavailable")); }
    while unsafe { *cursor } != 0 {
        let unit = unsafe { *cursor } as u32;
        full.push(char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid replay user directory"))?);
        cursor = unsafe { cursor.add(1) };
    }
    for &unit in name {
        if unit == 0 { break; }
        full.push(char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid replay filename"))?);
    }
    full.push_str(".rep");
    Ok(PathBuf::from(full))
}

/// Initializes only the fields initialized by `FReplay::FReplay`.
///
/// # Safety
/// `replay` must point to writable, zero-initialized FReplay FString members.
pub unsafe fn construct(_access: CoreAccess<'_>, replay: *mut FReplay) -> RuntimeResult<()> {
    if replay.is_null() { return Err(RuntimeError::new("Null FReplay")); }
    unsafe {
        (*replay).file_ar = ptr::null_mut();
        (*replay).b_recording = 0;
        (*replay).b_in_replay = 0;
        (*replay).b_paused = 0;
    }
    Ok(())
}

/// Destroys the archive before FString members, matching native reverse member order.
///
/// # Safety
/// `replay` is live, its FString members are Core-owned, and `file_ar` was created here.
pub unsafe fn destroy(access: CoreAccess<'_>, replay: *mut FReplay) -> RuntimeResult<()> {
    let mut first_error = unsafe { super::net_replay::destroy_with(replay, &OwnerOps(access)) }.err();
    unsafe {
        if !(*replay).file_ar.is_null() {
            // A failed close preserves the native slot, but this Rust-owned file
            // must be released before the FReplay storage goes away.
            if archive_from_handle((*replay).file_ar).is_some() {
                let raw = (*replay).file_ar;
                drop(Box::from_raw(raw.map_addr(|address| address & !ARCHIVE_TAG_MASK).cast::<ReplayArchive>()));
                (*replay).file_ar = ptr::null_mut();
            }
        }
        if let Err(error) = access.destroy_owned_fstring(ptr::addr_of_mut!((*replay).url_str)) {
            if first_error.is_none() { first_error = Some(error); }
        }
        if let Err(error) = access.destroy_owned_fstring(ptr::addr_of_mut!((*replay).file_name)) {
            if first_error.is_none() { first_error = Some(error); }
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// Opens a replay file under `appUserDir()` using native `.rep` name concatenation.
///
/// # Safety
/// `replay`, input name storage, and optional URL storage must remain valid for this call.
pub unsafe fn open_file(access: CoreAccess<'_>, replay: *mut FReplay, input_name: *const u32, record: bool, start_url: *const u32) -> RuntimeResult<bool> {
    let opened = unsafe {
        super::net_replay::open_file_with(
            replay,
            &OwnerOps(access),
            input_name.cast(),
            record,
            start_url.cast(),
            ptr::null_mut(),
        )
    }?;
    if opened && !record {
        let archive = unsafe { archive_from_handle((*replay).file_ar) }
            .ok_or_else(|| RuntimeError::new("Replay archive unavailable after opening"))?;
        let url = unsafe { string_slice(ptr::addr_of!((*replay).url_str))? };
        archive.url = url.iter().take_while(|&&unit| unit != 0)
            .filter_map(|&unit| char::from_u32(unit)).collect();
        archive.seeded = true;
    }
    Ok(opened)
}

fn path_to_units(path: &std::path::Path) -> Vec<u32> {
    let mut units: Vec<u32> = path.to_string_lossy().chars().map(u32::from).collect();
    units.push(0);
    units
}

/// Stops an active replay, leaving `b_recording` unchanged as native does.
/// # Safety
/// `replay` is a live FReplay whose archive belongs to this owner.
pub unsafe fn stop(access: CoreAccess<'_>, replay: *mut FReplay) -> RuntimeResult<bool> {
    unsafe { super::net_replay::stop_with(replay, &OwnerOps(access)) }
}

/// Serializes a frame delta and updates replay frame/pause counters.
/// # Safety
/// `replay` and `tick_delta` are live writable values; the archive is owner-managed.
pub unsafe fn serialize_frame_tick(access: CoreAccess<'_>, replay: *mut FReplay, tick_delta: *mut f32) -> RuntimeResult<bool> {
    unsafe { super::net_replay::serialize_tick_with(replay, &OwnerOps(access), tick_delta) }
}

fn write_input(file: &mut BufWriter<File>, event: FReplayInputEvent) -> RuntimeResult<()> {
    let key = event.i_key as u8;
    let state = super::net_replay_wire::encode_state(event.state as u8, event.delta);
    write_all(file, &[key, state])?;
    if super::net_replay_wire::has_delta(state) { write_all(file, &event.delta.to_le_bytes())?; }
    Ok(())
}

fn read_input(file: &mut BufReader<File>, event: *mut FReplayInputEvent) -> RuntimeResult<()> {
    let mut wire = [0; 2];
    read_exact(file, &mut wire)?;
    let state = wire[1];
    let delta = if super::net_replay_wire::has_delta(state) {
        let mut bytes = [0; 4];
        read_exact(file, &mut bytes)?;
        f32::from_le_bytes(bytes)
    } else { 0.0 };
    unsafe {
        (*event).i_key = i32::from(wire[0]);
        (*event).state = i32::from(super::net_replay_wire::decode_action(state));
        (*event).delta = delta;
    }
    Ok(())
}

/// Serializes an input event, returning true while input belongs to the replay frame.
/// # Safety
/// `replay` and `event` are live; an active archive belongs to this owner.
pub unsafe fn serialize_frame_input(access: CoreAccess<'_>, replay: *mut FReplay, event: *mut FReplayInputEvent) -> RuntimeResult<bool> {
    unsafe { super::net_replay::serialize_input_with(replay, &OwnerOps(access), event) }
}

/// Serializes a write-only event using the native input-event wire format.
/// # Safety
/// `replay` is a live FReplay with an owner-managed archive when active.
pub unsafe fn serialize_frame_input_write(access: CoreAccess<'_>, replay: *mut FReplay, key: i32, state: i32, delta: f32) -> RuntimeResult<bool> {
    let mut event = FReplayInputEvent { i_key: key, state, delta };
    unsafe { serialize_frame_input(access, replay, &mut event) }
}

/// Returns whether the active archive is recording.
pub unsafe fn recording(replay: *const FReplay) -> bool { unsafe { !(*replay).file_ar.is_null() && (*replay).b_recording != 0 } }
/// Returns whether the active archive is replaying.
pub unsafe fn replaying(replay: *const FReplay) -> bool { unsafe { !(*replay).file_ar.is_null() && (*replay).b_recording == 0 } }
/// Returns whether the current input event belongs to replay playback.
///
/// # Safety
/// `replay` points to a live `FReplay`.
pub unsafe fn in_replay(replay: *const FReplay) -> bool { unsafe { (*replay).b_in_replay != 0 } }

/// Returns the current frame number.
///
/// # Safety
/// `replay` points to a live `FReplay`.
pub unsafe fn frame_num(replay: *const FReplay) -> i32 { unsafe { (*replay).frame_num } }

/// Sets pause state and the remaining replay pause count.
///
/// # Safety
/// `replay` points to a live `FReplay`.
pub unsafe fn pause(replay: *mut FReplay, paused: bool, count: i32) {
    unsafe { (*replay).b_paused = paused as u8; (*replay).pause_count = count; }
}

/// Returns native pause state (`bPaused && PauseCount == 0`).
///
/// # Safety
/// `replay` points to a live `FReplay`.
pub unsafe fn paused(replay: *const FReplay) -> bool {
    unsafe { (*replay).b_paused != 0 && (*replay).pause_count == 0 }
}

#[cfg(test)]
mod render_host_contracts {
    use super::*;

    #[test]
    fn replay_consumption_reports_decoded_frames_only_after_real_eof() {
        let owner = crate::render_owner::render_test_support::owner("replay-consumption");
        let access = owner.access();
        let stem: Vec<u32> = "observed_replay".chars().map(u32::from).chain([0]).collect();
        let path = replay_path(access, &stem).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        {
            let mut file = BufWriter::new(File::create(&path).unwrap());
            fstring_to_wire(&"Entry.unr".chars().map(u32::from).collect::<Vec<_>>(), &mut file).unwrap();
            write_input(&mut file, FReplayInputEvent { i_key: 250, state: 4, delta: 0.05 }).unwrap();
            write_input(&mut file, FReplayInputEvent { i_key: 87, state: 1, delta: 0.0 }).unwrap();
            write_input(&mut file, FReplayInputEvent { i_key: 250, state: 4, delta: 0.1 }).unwrap();
            file.flush().unwrap();
        }
        unsafe {
            let mut replay: FReplay = std::mem::zeroed();
            construct(access, &raw mut replay).unwrap();
            assert!(open_file(access, &raw mut replay, stem.as_ptr(), false, ptr::null()).unwrap());
            let raw = replay.file_ar;
            assert!(!archive_from_handle(raw).unwrap().completion_reported);
            let mut event: FReplayInputEvent = std::mem::zeroed();
            assert!(serialize_frame_input(access, &raw mut replay, &raw mut event).unwrap());
            assert_eq!((event.i_key, event.state), (87, 1));
            assert!(!archive_from_handle(raw).unwrap().completion_reported);
            assert!(!serialize_frame_input(access, &raw mut replay, &raw mut event).unwrap());
            assert_eq!(replay.next_tick.to_bits(), 0.1_f32.to_bits());
            let archive = archive_from_handle(raw).unwrap();
            assert_eq!(archive.consumed_frames, 2);
            assert_eq!(archive.first_tick_bits, Some(0.05_f32.to_bits()));
            assert!(!archive.completion_reported);
            assert!(!serialize_frame_input(access, &raw mut replay, &raw mut event).unwrap());
            assert!(replay.file_ar.is_null(), "EOF closes the actual reader");
            destroy(access, &raw mut replay).unwrap();

            // A complete first delimiter followed by a truncated input delta
            // must propagate the read error, not claim full consumption.
            {
                let mut file = BufWriter::new(File::create(&path).unwrap());
                fstring_to_wire(&"Entry.unr".chars().map(u32::from).collect::<Vec<_>>(), &mut file).unwrap();
                write_input(&mut file, FReplayInputEvent { i_key: 250, state: 4, delta: 0.05 }).unwrap();
                write_all(&mut file, &[250, 0x84, 0x00]).unwrap();
                file.flush().unwrap();
            }
            let mut replay: FReplay = std::mem::zeroed();
            construct(access, &raw mut replay).unwrap();
            assert!(open_file(access, &raw mut replay, stem.as_ptr(), false, ptr::null()).unwrap());
            assert!(serialize_frame_input(access, &raw mut replay, &raw mut event).is_err());
            let archive = archive_from_handle(replay.file_ar).unwrap();
            assert_eq!(archive.consumed_frames, 1);
            assert!(!archive.completion_reported);
            destroy(access, &raw mut replay).unwrap();
        }
    }
}
