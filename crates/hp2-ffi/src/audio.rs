//! `extern "C"` exports for the ALAudio port (`hp2_audio`).
//!
//! Engine-owned state stays in C++. The stream entry point takes the
//! OpenAL and `GFileStream` calls as C callbacks, so this crate never links
//! OpenAL and never dereferences an engine pointer.

use std::ffi::c_void;
use std::panic::AssertUnwindSafe;

use hp2_audio::stream::{BufferState, MAX_BUFFERS_PER_STREAM, StreamState, tick};
use hp2_ffi_support::guard;

/// Mirrors the three `ALSource` fields `hp2_audio::priority::
/// SourceSlot` needs (`crates/hp2-audio/src/priority.rs`). Declared
/// directly in this crate for the same cbindgen-parsing reason as
/// the other FFI mirror structs.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiAudioSourceSlot {
    pub has_sound: bool,
    pub id: i32,
    pub priority: f32,
}

/// Replaces `UALAudioSubsystem::FindLeastPriorityNonTalkSource`'s
/// body (`ALAudioSubsystem.cpp:2085-2101`). `sources` must point to
/// `count` contiguous `FfiAudioSourceSlot`s; the caller builds this
/// array from its own `TArray<ALSource>` immediately before the call
/// (no `ALSource` field Rust does not need crosses the boundary).
///
/// # Safety
/// `sources` must be a valid, aligned pointer to at least `count`
/// readable `FfiAudioSourceSlot`s (or `count == 0`, in which case
/// `sources` may be dangling).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_find_least_priority_non_talk_source(
    sources: *const FfiAudioSourceSlot,
    count: usize,
) -> i32 {
    guard(
        "hp2_rs_audio_find_least_priority_non_talk_source",
        -1,
        move || unsafe {
            let slots: Vec<hp2_audio::priority::SourceSlot> = if count == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(sources, count)
                    .iter()
                    .map(|s| hp2_audio::priority::SourceSlot {
                        has_sound: s.has_sound,
                        id: s.id,
                        priority: s.priority,
                    })
                    .collect()
            };
            hp2_audio::priority::find_least_priority_non_talk_source(&slots)
        },
    )
}

/// Replaces `UALAudioSubsystem::SoundPriority`'s body
/// (`ALAudioSubsystem.cpp:1946-1961`). `dist_squared` is the caller's
/// own `FDistSquared(Location, ViewTarget->Location)`; `flags` is the
/// sound's `WorkingFlags`/`Flags` bits as-is.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_sound_priority(
    dist_squared: f32,
    radius: f32,
    audio_max_radius_multiplier: f32,
    volume: f32,
    flags: i32,
) -> f32 {
    guard("hp2_rs_audio_sound_priority", 0.0, move || {
        hp2_audio::priority::sound_priority(
            dist_squared,
            radius,
            audio_max_radius_multiplier,
            volume,
            flags,
        )
    })
}

/// Mirrors `hp2_audio::priority::LeastImportantSlot`
/// (`crates/hp2-audio/src/priority.rs`). `sound` is a `USound*`'s raw
/// identity bits (`(uintptr_t)Source.Sound` in C++); this crate never
/// dereferences it, only compares it for equality.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiLeastImportantSlot {
    pub sound: usize,
    pub id: i32,
    pub dist_squared: f32,
    pub volume: f32,
    pub radius: f32,
    pub flags: i32,
}

/// Replaces `UALAudioSubsystem::FindLeastImportantSound`'s body
/// (`ALAudioSubsystem.cpp:2105-2156`). `slots` must point to `count`
/// contiguous `FfiLeastImportantSlot`s in the same order as the real
/// C++'s `Sources` array (the "most frequent sound" tie-break depends
/// on that order -- see `find_least_important_sound`'s own doc).
/// `incoming_sound` is the raw identity bits of the `USound*` about
/// to play.
///
/// # Safety
/// `slots` must be a valid, aligned pointer to at least `count`
/// readable `FfiLeastImportantSlot`s (or `count == 0`, in which case
/// `slots` may be dangling).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_find_least_important_sound(
    slots: *const FfiLeastImportantSlot,
    count: usize,
    incoming_sound: usize,
    audio_max_radius_multiplier: f32,
) -> i32 {
    guard(
        "hp2_rs_audio_find_least_important_sound",
        -1,
        move || unsafe {
            let owned: Vec<hp2_audio::priority::LeastImportantSlot> = if count == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(slots, count)
                    .iter()
                    .map(|s| hp2_audio::priority::LeastImportantSlot {
                        sound: s.sound,
                        id: s.id,
                        dist_squared: s.dist_squared,
                        volume: s.volume,
                        radius: s.radius,
                        flags: s.flags,
                    })
                    .collect()
            };
            hp2_audio::priority::find_least_important_sound(
                &owned,
                incoming_sound,
                audio_max_radius_multiplier,
            )
        },
    )
}

/// Copy of the `ALStream` fields one `UpdateStreamingSounds` pass reads
/// and writes (`ALAudioSubsystem.h`). C++ copies `Buffer`, `RefillData`,
/// `BufferState`, `NumBuffers`, `Alive`, `Reset` in before the call and
/// copies `BufferState`, `Alive`, `Reset` back after a successful call.
/// `refill` holds `BYTE*` values as opaque `usize`. `state` holds
/// `EALStreamBufferState` as its C++ `0..4` byte. `alive`/`reset` are
/// `UBOOL` as `0`/`1`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiAudioStream {
    pub buffers: [u32; 3],
    pub refill: [usize; 3],
    pub state: [u8; 3],
    pub num_buffers: i32,
    pub alive: u8,
    pub reset: u8,
}

/// C++ calls the state machine uses, passed as function pointers so
/// `hp2_audio::stream::tick` stays free of OpenAL and `GFileStream`.
/// `ctx` is passed back to every callback unchanged. Each callback is
/// one C++ call site, invoked in the same order as the original loop.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiAudioStreamOps {
    /// `alSourceUnqueueBuffers(Source, 1, &b)` followed by
    /// `alError("unqueueing")`. Returns `b`.
    pub unqueue: unsafe extern "C" fn(ctx: *mut c_void) -> u32,
    /// `GFileStream->PopCompletedChunk(Id-1, dest, terminal)`. Returns
    /// `1` and writes `*dest` and `*terminal` (`0`/`1`) when a chunk was
    /// popped, `0` otherwise.
    pub pop_completed:
        unsafe extern "C" fn(ctx: *mut c_void, dest: *mut usize, terminal: *mut u8) -> u8,
    /// `alBufferData` + `alError("bufferdata")`, then
    /// `alSourceQueueBuffers` + `alError("queueing")` for
    /// `Buffer[buffer_index]`.
    pub queue: unsafe extern "C" fn(ctx: *mut c_void, buffer_index: u32),
    /// `GFileStream->RequestChunk(Id-1, dest)`. Returns `1` if accepted.
    pub request: unsafe extern "C" fn(ctx: *mut c_void, dest: usize) -> u8,
    /// Passed back to every callback above.
    pub ctx: *mut c_void,
}

/// Runs one `UpdateStreamingSounds` pass (`ALAudioSubsystem.cpp:1495-1597`)
/// on a copy of the C++ stream state, using `ops` for every OpenAL and
/// `GFileStream` call.
///
/// `processed` is `alGetSourcei(AL_BUFFERS_PROCESSED)`. Returns `1` when
/// the pass requested `alSourcePlay` (`Reset` was set), `0` when it did
/// not, and `-1` on a state-machine error. On `-1`, `stream` is not
/// written, and the C++ `check` that corresponds to the error fires.
///
/// # Safety
/// `stream` must point to a live, writable `FfiAudioStream`. Every
/// function pointer in `ops` must be valid, and `ops.ctx` must be what
/// those functions expect.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_stream_tick(
    stream: *mut FfiAudioStream,
    processed: i32,
    ops: FfiAudioStreamOps,
) -> i32 {
    guard(
        "hp2_rs_audio_stream_tick",
        -1,
        AssertUnwindSafe(move || unsafe {
            let s = &mut *stream;
            if s.num_buffers < 0 || s.num_buffers as usize > MAX_BUFFERS_PER_STREAM {
                return -1;
            }
            let mut state = [BufferState::QueuedPending; MAX_BUFFERS_PER_STREAM];
            for (dst, &byte) in state.iter_mut().zip(s.state.iter()) {
                match BufferState::from_u8(byte) {
                    Some(value) => *dst = value,
                    None => return -1,
                }
            }
            let mut copy = StreamState {
                buffers: s.buffers,
                refill: s.refill,
                state,
                num_buffers: s.num_buffers as usize,
                alive: s.alive != 0,
                reset: s.reset != 0,
            };
            let ctx = ops.ctx;
            let outcome = tick(
                &mut copy,
                processed,
                || (ops.unqueue)(ctx),
                || {
                    let mut dest = 0usize;
                    let mut terminal = 0u8;
                    if (ops.pop_completed)(ctx, &mut dest, &mut terminal) != 0 {
                        Some((dest, terminal != 0))
                    } else {
                        None
                    }
                },
                |index| (ops.queue)(ctx, index as u32),
                |dest| (ops.request)(ctx, dest) != 0,
            );
            match outcome {
                Ok(outcome) => {
                    s.state = copy.state.map(|value| value as u8);
                    s.alive = u8::from(copy.alive);
                    s.reset = u8::from(copy.reset);
                    i32::from(outcome.played)
                }
                Err(_) => -1,
            }
        }),
    )
}

/// Mirror of C++ `ALAmbient` (`ALAudioSubsystem.h`) including its `Id`.
/// C++ owns the array; Rust only reorders it in place.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiAmbientSound {
    pub actor: *mut c_void,
    pub priority: f32,
    pub flags: i32,
    pub id: i32,
}

/// Replaces `Sort(&AmbientSounds(0), Num)` in `HandleAmbience`
/// (`ALAudioSubsystem.cpp:1492`) with the same UE1 `Sort` algorithm and
/// `Compare` (`:1270`). Highest priority first; tie order is the original's.
///
/// # Safety
/// `sounds` must point to `num` writable `FfiAmbientSound`s. A null pointer
/// or `num < 2` is a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_sort_ambient(sounds: *mut FfiAmbientSound, num: i32) {
    guard(
        "hp2_rs_audio_sort_ambient",
        (),
        AssertUnwindSafe(move || unsafe {
            if sounds.is_null() || num < 2 {
                return;
            }
            let items = std::slice::from_raw_parts_mut(sounds, num as usize);
            hp2_audio::ambient::sort_by_priority(items, |s| s.priority);
        }),
    )
}

/// `Actor->GetIndex()*16 + SLOT_Ambient*2` (`ALAudioSubsystem.cpp:1448`).
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_ambient_source_id(actor_index: i32) -> i32 {
    guard("hp2_rs_audio_ambient_source_id", 0, move || {
        hp2_audio::ambient::ambient_source_id(actor_index)
    })
}

/// Start flags for an ambient sound (`ALAudioSubsystem.cpp:1460,1480`).
/// `owned` is nonzero for an actor the listener owns.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_ambient_flags(owned: u8) -> i32 {
    guard("hp2_rs_audio_ambient_flags", 0, move || {
        hp2_audio::ambient::ambient_flags(owned != 0)
    })
}

/// `HandleAmbience` admission (`ALAudioSubsystem.cpp:1455-1482`). Returns
/// `1` to admit the actor, `0` to skip it. `owned` nonzero bypasses distance.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_ambient_admitted(
    owned: u8,
    dist_squared: f32,
    radius: f32,
    radius_multiplier: f32,
    max_radius_multiplier: f32,
) -> u8 {
    guard("hp2_rs_audio_ambient_admitted", 0, move || {
        u8::from(hp2_audio::ambient::ambient_admitted(
            owned != 0,
            dist_squared,
            radius,
            radius_multiplier,
            max_radius_multiplier,
        ))
    })
}

/// `UpdateAmbience` stop rule (`ALAudioSubsystem.cpp:1514-1524`). Returns
/// `1` when the playing ambient source must stop, `0` to keep it.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_ambient_out_of_range(
    dist_squared: f32,
    zone_radius: f32,
    max_radius_multiplier: f32,
) -> u8 {
    guard("hp2_rs_audio_ambient_out_of_range", 0, move || {
        u8::from(hp2_audio::ambient::ambient_out_of_range(
            dist_squared,
            zone_radius,
            max_radius_multiplier,
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the byte layout C++ copies into (`FfiAudioStream` in
    /// `hp2_rs.h`). Matching `static_assert`s live in
    /// `ALAudioSubsystem.cpp`.
    #[test]
    fn stream_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiAudioStream>(), 56);
        assert_eq!(std::mem::offset_of!(FfiAudioStream, buffers), 0);
        assert_eq!(std::mem::offset_of!(FfiAudioStream, refill), 16);
        assert_eq!(std::mem::offset_of!(FfiAudioStream, state), 40);
        assert_eq!(std::mem::offset_of!(FfiAudioStream, num_buffers), 44);
        assert_eq!(std::mem::offset_of!(FfiAudioStream, alive), 48);
        assert_eq!(std::mem::offset_of!(FfiAudioStream, reset), 49);
    }

    /// Pins the byte layout C++ copies into (`FfiAmbientSound` in
    /// `hp2_rs.h`), matching `static_assert`s in `ALAudioSubsystem.cpp`.
    #[test]
    fn ambient_mirror_layout() {
        let ptr = std::mem::size_of::<*mut c_void>();
        assert_eq!(std::mem::offset_of!(FfiAmbientSound, actor), 0);
        assert_eq!(std::mem::offset_of!(FfiAmbientSound, priority), ptr);
        assert_eq!(std::mem::offset_of!(FfiAmbientSound, flags), ptr + 4);
        assert_eq!(std::mem::offset_of!(FfiAmbientSound, id), ptr + 8);
        assert_eq!(
            std::mem::size_of::<FfiAmbientSound>(),
            (ptr + 12).next_multiple_of(ptr)
        );
    }
}
