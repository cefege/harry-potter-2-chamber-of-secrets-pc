//! Port of `HarryPotter2/Unreal/Engine/Src/UnReplay.cpp` (`FReplay`).
//!
//! Rust owns the shared control flow and the `FReplay` state (mirror
//! `hp2_abi::engine::generated::FReplay`). The archive adapter is source-specific:
//! the native wrapper uses [`Hp2NetReplayServices`], while canonical
//! `replay_owner` supplies Core-owned FString storage, buffered files, logging,
//! and RNG directly. The native service table mirrors the C++ expressions in
//! `Hp2NetReplayServices.cpp` and its header; the const asserts below pin its
//! size and field offsets on the Rust side.
//!
//! The status/exception contract below applies to the native service adapter:
//! status 0 means the C++ expression completed, while 1 means it threw (the
//! thunk captured the exception with `Hp2FfiCall`). The port returns [`Thrown`]
//! at once and makes no further service call on that path, so the state it leaves
//! behind matches the state the C++ throw left at the same statement. The C++
//! shell then rethrows the captured exception.

use std::ffi::c_void;
use std::mem::{offset_of, size_of};
use std::ptr::{addr_of_mut, null_mut};

use hp2_abi::engine::generated::{FReplay, FReplayInputEvent};

/// `EInputKey::IK_Play` (`Engine/Inc/EngineClasses.h`).
pub const IK_PLAY: i32 = 250;
/// `EInputAction::IST_Axis` (`Engine/Inc/EngineClasses.h`).
pub const IST_AXIS: i32 = 4;

/// A services call threw on the C++ side (status 1). The exception itself stays
/// in the C++ `Hp2PendingException` slot and is rethrown by the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrown;

/// Result of a replay operation that can make services calls.
pub type Res<T> = Result<T, Thrown>;

/// Maps a services status (0 completed, nonzero thrown) to `Res`.
#[inline]
fn done(status: i32) -> Res<()> {
    if status == 0 { Ok(()) } else { Err(Thrown) }
}
/// Archive primitives used by the shared `FReplay` state machine.
pub(crate) trait ReplayOps {
    type Error;
    /// # Safety
    /// The replay, log device, and initialized filename member are live.
    unsafe fn log_already_recording(&self, replay: *mut FReplay, log: *mut c_void) -> Result<(), Self::Error>;
    /// # Safety
    /// The replay, log device, and initialized filename member are live.
    unsafe fn log_already_replaying(&self, replay: *mut FReplay, log: *mut c_void) -> Result<(), Self::Error>;
    /// # Safety
    /// The replay member and input name are valid for the implementation.
    unsafe fn assign_file_name(&self, replay: *mut FReplay, input_name: *const c_void) -> Result<(), Self::Error>;
    /// # Safety
    /// The filename member is initialized and `record` selects the matching file mode.
    unsafe fn create_file(&self, replay: *mut FReplay, record: bool) -> Result<*mut c_void, Self::Error>;
    /// # Safety
    /// The replay URL member and optional start URL obey the implementation contract.
    unsafe fn assign_url(&self, replay: *mut FReplay, start_url: *const c_void) -> Result<(), Self::Error>;
    /// # Safety
    /// `archive` and the replay's initialized URL member are live.
    unsafe fn serialize_url(&self, archive: *mut c_void, replay: *mut FReplay) -> Result<(), Self::Error>;
    /// # Safety
    /// The replay, log device, and initialized filename member are live.
    unsafe fn log_cannot_open(&self, replay: *mut FReplay, log: *mut c_void, record: bool) -> Result<(), Self::Error>;
    unsafe fn reseed(&self) -> Result<(), Self::Error>;
    /// # Safety
    /// `archive` is this implementation's live archive handle.
    unsafe fn delete(&self, archive: *mut c_void) -> Result<(), Self::Error>;
    /// # Safety
    /// `archive` is this implementation's live readable archive.
    unsafe fn at_end(&self, archive: *mut c_void) -> Result<bool, Self::Error>;
    /// # Safety
    /// `archive` and `event` are live values accepted by this implementation.
    unsafe fn serialize_input(&self, archive: *mut c_void, event: *mut FReplayInputEvent) -> Result<(), Self::Error>;
    /// # Safety
    /// `archive` is this implementation's live writable archive.
    unsafe fn flush(&self, archive: *mut c_void) -> Result<(), Self::Error>;
}

pub(crate) unsafe fn open_file_with<O: ReplayOps>(
    replay: *mut FReplay,
    ops: &O,
    input_name: *const c_void,
    record: bool,
    start_url: *const c_void,
    log: *mut c_void,
) -> Result<bool, O::Error> {
    unsafe {
        if recording(replay) {
            ops.log_already_recording(replay, log)?;
            return Ok(false);
        } else if replaying(replay) {
            ops.log_already_replaying(replay, log)?;
            return Ok(false);
        }
        ops.assign_file_name(replay, input_name)?;
        let archive = ops.create_file(replay, record)?;
        (*replay).file_ar = archive;
        if archive.is_null() {
            ops.log_cannot_open(replay, log, record)?;
            return Ok(false);
        }
        (*replay).b_recording = record as u8;
        (*replay).b_in_replay = 0;
        (*replay).frame_num = 0;
        ops.assign_url(replay, start_url)?;
        ops.serialize_url(archive, replay)?;
        if !record {
            let mut event = FReplayInputEvent { i_key: 0, state: 0, delta: 0.0 };
            serialize_input_with(replay, ops, addr_of_mut!(event))?;
        }
        ops.reseed()?;
    }
    Ok(true)
}


pub(crate) unsafe fn destroy_with<O: ReplayOps>(replay: *mut FReplay, ops: &O) -> Result<(), O::Error> {
    unsafe {
        ops.delete((*replay).file_ar)?;
        (*replay).file_ar = null_mut();
    }
    Ok(())
}

pub(crate) unsafe fn stop_with<O: ReplayOps>(replay: *mut FReplay, ops: &O) -> Result<bool, O::Error> {
    unsafe {
        ops.delete((*replay).file_ar)?;
        (*replay).file_ar = null_mut();
        (*replay).b_paused = 0;
    }
    Ok(true)
}

pub(crate) unsafe fn serialize_tick_with<O: ReplayOps>(
    replay: *mut FReplay,
    ops: &O,
    tick_delta: *mut f32,
) -> Result<bool, O::Error> {
    unsafe {
        if recording(replay) {
            let mut event = FReplayInputEvent { i_key: IK_PLAY, state: IST_AXIS, delta: *tick_delta };
            ops.serialize_input((*replay).file_ar, addr_of_mut!(event))?;
            ops.flush((*replay).file_ar)?;
        } else if replaying(replay) {
            *tick_delta = (*replay).next_tick;
            if (*replay).pause_count > 0 { (*replay).pause_count -= 1; }
        }
        (*replay).frame_num = (*replay).frame_num.wrapping_add(1);
    }
    Ok(true)
}

pub(crate) unsafe fn serialize_input_with<O: ReplayOps>(
    replay: *mut FReplay,
    ops: &O,
    event: *mut FReplayInputEvent,
) -> Result<bool, O::Error> {
    unsafe {
        if (*replay).file_ar.is_null() { return Ok(false); }
        if replaying(replay) {
            if ops.at_end((*replay).file_ar)? {
                ops.delete((*replay).file_ar)?;
                (*replay).file_ar = null_mut();
                (*replay).b_in_replay = 0;
                return Ok(false);
            }
        }
        ops.serialize_input((*replay).file_ar, event)?;
        if replaying(replay) && (*event).i_key == IK_PLAY && (*event).state == IST_AXIS {
            (*replay).next_tick = (*event).delta;
            (*replay).b_in_replay = 0;
            return Ok(false);
        }
        (*replay).b_in_replay = 1;
        Ok(true)
    }
}

struct ServiceOps<'a>(&'a Hp2NetReplayServices);
impl ReplayOps for ServiceOps<'_> {
    type Error = Thrown;

    unsafe fn log_already_recording(&self, replay: *mut FReplay, log: *mut c_void) -> Res<()> {
        let name = unsafe { addr_of_mut!((*replay).file_name).cast::<c_void>() };
        done(unsafe { (self.0.replay_log_already_recording)(log, name) })
    }
    unsafe fn log_already_replaying(&self, replay: *mut FReplay, log: *mut c_void) -> Res<()> {
        let name = unsafe { addr_of_mut!((*replay).file_name).cast::<c_void>() };
        done(unsafe { (self.0.replay_log_already_replaying)(log, name) })
    }
    unsafe fn assign_file_name(&self, replay: *mut FReplay, input: *const c_void) -> Res<()> {
        let name = unsafe { addr_of_mut!((*replay).file_name).cast::<c_void>() };
        done(unsafe { (self.0.replay_file_name_assign)(name, input) })
    }
    unsafe fn create_file(&self, replay: *mut FReplay, record: bool) -> Res<*mut c_void> {
        let name = unsafe { addr_of_mut!((*replay).file_name).cast::<c_void>() };
        let mut archive = null_mut();
        done(unsafe { (self.0.replay_create_file)(name, record as i32, &mut archive) })?;
        Ok(archive)
    }
    unsafe fn assign_url(&self, replay: *mut FReplay, start_url: *const c_void) -> Res<()> {
        let url = unsafe { addr_of_mut!((*replay).url_str).cast::<c_void>() };
        done(unsafe { (self.0.replay_url_assign)(url, start_url) })
    }
    unsafe fn serialize_url(&self, archive: *mut c_void, replay: *mut FReplay) -> Res<()> {
        let url = unsafe { addr_of_mut!((*replay).url_str).cast::<c_void>() };
        done(unsafe { (self.0.replay_archive_serialize_url)(archive, url) })
    }
    unsafe fn log_cannot_open(&self, replay: *mut FReplay, log: *mut c_void, record: bool) -> Res<()> {
        let name = unsafe { addr_of_mut!((*replay).file_name).cast::<c_void>() };
        done(unsafe { (self.0.replay_log_cannot_open)(log, record as i32, name) })
    }
    unsafe fn reseed(&self) -> Res<()> {
        done(unsafe { (self.0.replay_reseed_for_replay)() })
    }
    unsafe fn delete(&self, archive: *mut c_void) -> Res<()> {
        done(unsafe { (self.0.replay_archive_delete)(archive) })
    }
    unsafe fn at_end(&self, archive: *mut c_void) -> Res<bool> {
        let mut value = 0;
        done(unsafe { (self.0.replay_archive_at_end)(archive, &mut value) })?;
        Ok(value != 0)
    }
    unsafe fn serialize_input(&self, archive: *mut c_void, event: *mut FReplayInputEvent) -> Res<()> {
        done(unsafe { (self.0.replay_archive_serialize_input)(archive, event.cast()) })
    }
    unsafe fn flush(&self, archive: *mut c_void) -> Res<()> {
        done(unsafe { (self.0.replay_archive_flush)(archive) })
    }
}

/// Engine callbacks used by the replay port. Field order and types must match
/// `Hp2NetReplayServices` in `Hp2NetReplayServices.h`. Each entry returns 0 on
/// completion and 1 when the C++ side threw; results go through out-pointers.
#[repr(C)]
pub struct Hp2NetReplayServices {
    /// `Ar.Logf(TEXT("Already recording: %s"), *FileName)`.
    pub replay_log_already_recording:
        unsafe extern "C" fn(ar: *mut c_void, file_name: *const c_void) -> i32,
    /// `Ar.Logf(TEXT("Already replaying: %s"), *FileName)`.
    pub replay_log_already_replaying:
        unsafe extern "C" fn(ar: *mut c_void, file_name: *const c_void) -> i32,
    /// `Ar.Logf(TEXT("Cannot %s file %s"), bRecord ? "create" : "open", *FileName)`.
    pub replay_log_cannot_open:
        unsafe extern "C" fn(ar: *mut c_void, record: i32, file_name: *const c_void) -> i32,
    /// `FileName = FString(appUserDir()) + InName + TEXT(".rep")`.
    pub replay_file_name_assign:
        unsafe extern "C" fn(file_name: *mut c_void, in_name: *const c_void) -> i32,
    /// `URLStr = StartURL`. A NULL `start_url` with non-empty `URLStr` crashes in
    /// the C++ `operator=` (raw deref), as the original does.
    pub replay_url_assign:
        unsafe extern "C" fn(url_str: *mut c_void, start_url: *const c_void) -> i32,
    /// `*out = bRecord ? GFileManager->CreateFileWriter(*FileName) : GFileManager->CreateFileReader(*FileName)`.
    pub replay_create_file:
        unsafe extern "C" fn(file_name: *const c_void, record: i32, out: *mut *mut c_void) -> i32,
    /// `*FileAr << URLStr`.
    pub replay_archive_serialize_url:
        unsafe extern "C" fn(ar: *mut c_void, url_str: *mut c_void) -> i32,
    /// `*FileAr << IE` (`FReplay::FInputEvent`, `UnReplayWire.cpp`).
    pub replay_archive_serialize_input:
        unsafe extern "C" fn(ar: *mut c_void, ie: *mut c_void) -> i32,
    /// `*out = FileAr->AtEnd()`.
    pub replay_archive_at_end: unsafe extern "C" fn(ar: *mut c_void, out: *mut i32) -> i32,
    /// `FileAr->Flush()`.
    pub replay_archive_flush: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `delete FileAr` (NULL-safe, as C++ `delete`).
    pub replay_archive_delete: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    /// `srand(1); appResetRandTraceForReplay();`
    pub replay_reseed_for_replay: unsafe extern "C" fn() -> i32,
}

// 12 pointer-sized fields on LP64; the C++ mirror asserts the same size.
const _: () = assert!(size_of::<Hp2NetReplayServices>() == 96);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_log_already_recording) == 0);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_log_already_replaying) == 8);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_log_cannot_open) == 16);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_file_name_assign) == 24);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_url_assign) == 32);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_create_file) == 40);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_archive_serialize_url) == 48);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_archive_serialize_input) == 56);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_archive_at_end) == 64);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_archive_flush) == 72);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_archive_delete) == 80);
const _: () = assert!(offset_of!(Hp2NetReplayServices, replay_reseed_for_replay) == 88);

/// `FReplay::Recording()`: `FileAr && bRecording`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
#[inline]
pub unsafe fn recording(r: *const FReplay) -> bool {
    unsafe { !(*r).file_ar.is_null() && (*r).b_recording != 0 }
}

/// `FReplay::Replaying()`: `FileAr && !bRecording`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
#[inline]
pub unsafe fn replaying(r: *const FReplay) -> bool {
    unsafe { !(*r).file_ar.is_null() && (*r).b_recording == 0 }
}

/// `FReplay::InReplay()`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
#[inline]
pub unsafe fn in_replay(r: *const FReplay) -> bool {
    unsafe { (*r).b_in_replay != 0 }
}

/// `FReplay::Paused()`: `bPaused && PauseCount == 0`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
#[inline]
pub unsafe fn paused(r: *const FReplay) -> bool {
    unsafe { (*r).b_paused != 0 && (*r).pause_count == 0 }
}

/// `FReplay::GetFrameNum()`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
#[inline]
pub unsafe fn frame_num(r: *const FReplay) -> i32 {
    unsafe { (*r).frame_num }
}

/// `FReplay::Pause(bool, int)`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
pub unsafe fn pause(r: *mut FReplay, pause: bool, count: i32) {
    unsafe {
        (*r).b_paused = pause as u8;
        (*r).pause_count = count;
    }
}

/// `FReplay::~FReplay()` body: `delete FileAr; FileAr = NULL;`. The FString
/// members are destroyed by the C++ shell after this returns. On a thrown
/// `delete` the pointer is left as it was, as in C++.
///
/// # Safety
/// `r` must point to a live `FReplay`; `FileAr` must be NULL or a live archive.
pub unsafe fn dtor(r: *mut FReplay, svc: &Hp2NetReplayServices) -> Res<()> {
    unsafe { destroy_with(r, &ServiceOps(svc)) }
}

/// `FReplay::OpenFile`. `in_name` and `start_url` are `const TCHAR*` (`start_url`
/// may be NULL); `ar` is the `FOutputDevice&` used for log lines.
///
/// # Safety
/// `r` must point to a live `FReplay`; `in_name`, `start_url` and `ar` must be
/// valid for the C++ calls they are forwarded to.
pub unsafe fn open_file(
    r: *mut FReplay,
    svc: &Hp2NetReplayServices,
    in_name: *const c_void,
    record: bool,
    start_url: *const c_void,
    ar: *mut c_void,
) -> Res<bool> {
    unsafe { open_file_with(r, &ServiceOps(svc), in_name, record, start_url, ar) }
}

/// `FReplay::Stop()`.
///
/// # Safety
/// `r` must point to a live `FReplay`.
pub unsafe fn stop(r: *mut FReplay, svc: &Hp2NetReplayServices) -> Res<bool> {
    unsafe { stop_with(r, &ServiceOps(svc)) }
}

/// `FReplay::SerializeFrameTick(float& TickDelta)`. `tick_delta` is the C++
/// reference, passed as a pointer.
///
/// # Safety
/// `r` must point to a live `FReplay`; `tick_delta` must point to a live `f32`.
pub unsafe fn serialize_frame_tick(
    r: *mut FReplay,
    svc: &Hp2NetReplayServices,
    tick_delta: *mut f32,
) -> Res<bool> {
    unsafe { serialize_tick_with(r, &ServiceOps(svc), tick_delta) }
}

/// `FReplay::SerializeFrameInput(FInputEvent& IE)`.
///
/// # Safety
/// `r` must point to a live `FReplay`; `ie` must point to a live event.
pub unsafe fn serialize_frame_input(
    r: *mut FReplay,
    svc: &Hp2NetReplayServices,
    ie: *mut FReplayInputEvent,
) -> Res<bool> {
    unsafe { serialize_input_with(r, &ServiceOps(svc), ie) }
}

/// `FReplay::SerializeFrameInput(EInputKey, EInputAction, float)`: write-only
/// overload; builds the event then calls [`serialize_frame_input`].
///
/// # Safety
/// `r` must point to a live `FReplay`.
pub unsafe fn serialize_frame_input_write(
    r: *mut FReplay,
    svc: &Hp2NetReplayServices,
    key: i32,
    state: i32,
    delta: f32,
) -> Res<bool> {
    unsafe {
        let mut ie = FReplayInputEvent {
            i_key: key,
            state,
            delta,
        };
        serialize_frame_input(r, svc, &mut ie)
    }
}
