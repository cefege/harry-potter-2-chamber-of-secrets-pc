//! The engine and OpenAL surface that the Rust ALAudio subsystem calls through.
//!
//! Everything the subsystem needs from UE1 objects (`USound`, `AActor`,
//! `ULevel`, `UViewport`, `UAnimChannel`, `GFileStream`, the vorbis decoder,
//! the debug log, `appSeconds`) and from OpenAL/ALC is reached through this
//! trait. The C++ shell implements it by calling the engine directly, behind
//! `extern "C"` trampolines that catch C++ exceptions and report them as
//! `HostFailed`. Rust never swallows that error: every method that can throw
//! returns `HostResult`, and the subsystem stops at the first `Err` so no
//! further engine call runs after a throw, matching the unwinding of the
//! original C++.
//!
//! OpenAL calls are infallible in this sense (`alGetError` reports errors
//! without throwing), so they return plain values.

use crate::vec3::Vec3;

/// The engine threw while the host was running a call. The C++ shell keeps
/// the `std::exception_ptr` and rethrows it once the Rust export returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostFailed;

pub type HostResult<T> = Result<T, HostFailed>;

/// Which `debugf` category a log line goes to (`NAME_*` in the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogKind {
    Init,
    Exit,
    DevSound,
    DevMusic,
    Warning,
    /// `debugf` with no category (the `alError` messages).
    Log,
}

/// The `USound` fields the subsystem reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SoundInfo {
    pub core_flags: i32,
    pub working_flags: i32,
    /// `stricmp(FileType, "ogg") == 0`.
    pub is_ogg: bool,
    pub raw_num_samples: i32,
    pub raw_bits_per_sample: i32,
    pub raw_num_channels: i32,
    pub raw_sample_rate: i32,
}

/// `FWaveModInfo` as filled by `ReadWaveInfo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WaveInfo {
    pub sample_loops_num: i32,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub samples_per_sec: i32,
    /// Address of `SampleDataStart`. Valid until `sound_unload_data`.
    pub sample_data: usize,
    pub sample_data_size: i32,
}

/// What the ogg decoder reports (`hp2_rs_vorbis_*`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct VorbisInfo {
    pub pcm_total: i64,
    pub rate: i32,
    pub channels: i32,
    pub time_total: f64,
}

/// The `AActor` fields the subsystem reads.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ActorInfo {
    pub location: Vec3,
    pub velocity: Vec3,
    pub sound_pitch: u8,
    pub sound_volume: u8,
    pub sound_radius: f32,
    pub sound_occlusion: u8,
    pub ambient_sound: usize,
    pub index: i32,
    /// `Actor->GetLevel()`; 0 when the actor is in no level.
    pub level: usize,
    /// `Actor->Level`.
    pub actor_level: usize,
    /// `LightType == LT_None`.
    pub light_is_none: bool,
    pub light_brightness: u8,
}

/// The `UViewport` fields the subsystem reads, with `ViewTarget` resolved
/// as `Viewport->Actor->ViewTarget ? ViewTarget : Viewport->Actor`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ViewportInfo {
    pub realtime: bool,
    /// `Viewport->Actor->Level->Pauser != ""`.
    pub paused: bool,
    pub view_target: usize,
    /// `ViewTarget->GetLevel()`.
    pub view_target_level: usize,
    /// `ViewTarget->Level`.
    pub view_target_actor_level: usize,
    /// `Viewport->Actor->Level`.
    pub viewport_actor_level: usize,
    pub view_target_location: Vec3,
}

/// Everything the subsystem calls out to. See the module comment.
///
/// Snapshot lists (`*_count` then `*_fill`) replace iterators so that the
/// host never has to hand Rust a container. The order is stable between the
/// two calls because nothing mutates the list in between.
pub trait Host {
    // Logging, time, and configuration writes.
    fn log(&mut self, kind: LogKind, text: &str) -> HostResult<()>;
    /// `Ar.Logf` of the Exec call's output device.
    fn output(&mut self, text: &str) -> HostResult<()>;
    /// `appSeconds()` as its raw `FTime` tick count.
    fn app_seconds(&mut self) -> HostResult<i64>;
    /// `appErrorf(text)`: the C++ shell's implementation always fails, so this
    /// always returns `Err`. Rust returns `Err(HostFailed)` after the call.
    fn app_error(&mut self, text: &str) -> HostResult<()>;

    // USound.
    fn sound_info(&mut self, sound: usize) -> HostResult<SoundInfo>;
    /// `USound::Data.Num()`.
    fn sound_data_len(&mut self, sound: usize) -> HostResult<i32>;
    fn sound_name(&mut self, sound: usize) -> HostResult<String>;
    fn sound_path_name(&mut self, sound: usize) -> HostResult<String>;
    fn sound_filename(&mut self, sound: usize) -> HostResult<String>;
    fn sound_set_working_flags(&mut self, sound: usize, flags: i32) -> HostResult<()>;
    fn sound_set_handle(&mut self, sound: usize, handle: i32) -> HostResult<()>;
    /// `GetHandle(Id)`.
    fn sound_get_handle(&mut self, sound: usize, id: i32) -> HostResult<i32>;
    fn sound_set_duration(&mut self, sound: usize, duration: f32) -> HostResult<()>;
    /// Adds `delta` to `RefCount` and returns the new count.
    fn sound_add_ref(&mut self, sound: usize, delta: i32) -> HostResult<i32>;
    fn sound_is_registered(&mut self, sound: usize) -> HostResult<bool>;
    fn sound_add_stream_handle(&mut self, sound: usize, id: i32) -> HostResult<()>;
    fn sound_delete_stream_handle(&mut self, sound: usize, handle: i32) -> HostResult<()>;
    fn sound_bind_stream(&mut self, sound: usize, id: i32) -> HostResult<()>;
    fn sound_unbind_stream(&mut self, sound: usize, id: i32) -> HostResult<()>;
    fn sound_unbind_all_streams(&mut self, sound: usize) -> HostResult<()>;
    fn sound_delete_all_stream_handles(&mut self, sound: usize) -> HostResult<()>;
    /// Count of the `PlayingStreams` values of `sound`.
    fn sound_playing_stream_count(&mut self, sound: usize) -> HostResult<usize>;
    /// Writes the resolved stream handles (`OpenStreams(v) / 2` for each
    /// `PlayingStreams` entry `v`) into `out` in `PlayingStreams` order.
    fn sound_playing_stream_fill(&mut self, sound: usize, out: &mut [i32]) -> HostResult<()>;
    fn sound_load_data(&mut self, sound: usize) -> HostResult<()>;
    fn sound_unload_data(&mut self, sound: usize) -> HostResult<()>;
    /// `ReadWaveInfo(Data)` on the loaded data.
    fn sound_wave_info(&mut self, sound: usize) -> HostResult<WaveInfo>;
    /// Count of live `USound` objects (`TObjectIterator<USound>`).
    fn sound_count(&mut self) -> HostResult<usize>;
    fn sound_fill(&mut self, out: &mut [usize]) -> HostResult<()>;
    /// `new USound(filename, SF_Streaming|SF_Music)` with `FileType = "ogg"`.
    /// Returns 0 when allocation fails.
    fn music_sound_new(&mut self, filename: &str) -> HostResult<usize>;

    // GFileStream and the ogg decoder.
    fn vorbis_new(&mut self) -> HostResult<usize>;
    fn vorbis_free(&mut self, decoder: usize) -> HostResult<()>;
    fn vorbis_info(&mut self, decoder: usize) -> HostResult<VorbisInfo>;
    /// `CreateStream(*Sound->Filename, chunk, num, prime, type, decoder)`.
    /// `looping` selects `ST_OggLooping` over `ST_Ogg`. Returns the stream id
    /// (negative on failure).
    fn gfile_create_ogg_stream(
        &mut self,
        sound: usize,
        chunk: i32,
        num: i32,
        prime: &mut [u8],
        looping: bool,
        decoder: usize,
    ) -> HostResult<i32>;
    /// `CreateStream(&Sound->Data, num_samples, chunk, num, prime, type, NULL)`.
    fn gfile_create_xa_stream(
        &mut self,
        sound: usize,
        num_samples: i32,
        chunk: i32,
        num: i32,
        prime: &mut [u8],
        looping: bool,
    ) -> HostResult<i32>;
    fn gfile_destroy_stream(&mut self, id: i32) -> HostResult<()>;
    /// `RequestChunk(id, dest)`.
    fn gfile_request_chunk(&mut self, id: i32, dest: usize) -> HostResult<bool>;
    /// `PopCompletedChunk(id, dest, terminal)`: `Some((dest, terminal))`
    /// when a chunk completed.
    fn gfile_pop_completed_chunk(&mut self, id: i32) -> HostResult<Option<(usize, bool)>>;

    // Actors, levels, viewports.
    fn actor_info(&mut self, actor: usize) -> HostResult<ActorInfo>;
    fn actor_is_owned_by(&mut self, actor: usize, owner: usize) -> HostResult<bool>;
    /// `Level->IsAudibleAt(sound_location, listener_location, actor, occlusion)`.
    fn level_is_audible_at(
        &mut self,
        level: usize,
        sound_location: Vec3,
        listener_location: Vec3,
        actor: usize,
        occlusion: u8,
    ) -> HostResult<bool>;
    /// `level->Model->PointRegion(actor_level, location).ZoneNumber`.
    fn level_point_region_zone(
        &mut self,
        level: usize,
        actor_level: usize,
        location: Vec3,
    ) -> HostResult<i32>;
    /// `level->CalculateRadiusMultiplier(zone_a, zone_b)`.
    fn level_radius_multiplier(
        &mut self,
        level: usize,
        zone_a: i32,
        zone_b: i32,
    ) -> HostResult<f32>;
    fn viewport_info(&mut self, viewport: usize) -> HostResult<ViewportInfo>;
    /// `Level->Actors.Num()`.
    fn level_actor_count(&mut self, level: usize) -> HostResult<usize>;
    /// Writes `Level->Actors(i)` into `out` (0 for a NULL entry).
    fn level_actor_fill(&mut self, level: usize, out: &mut [usize]) -> HostResult<()>;

    // Lip sync (UAnimChannel). A channel of 0 means "none".
    fn lipsync_start(&mut self, actor: usize) -> HostResult<usize>;
    fn lipsync_set_frame(&mut self, channel: usize, sound: usize, elapsed: f32) -> HostResult<()>;
    fn lipsync_release(&mut self, channel: usize) -> HostResult<()>;

    // OpenAL and ALC. These cannot throw.
    fn al_get_error(&mut self) -> i32;
    fn al_gen_source(&mut self) -> u32;
    fn al_delete_source(&mut self, source: u32);
    fn al_gen_buffers(&mut self, out: &mut [u32]);
    fn al_delete_buffers(&mut self, buffers: &[u32]);
    fn al_buffer_data(&mut self, buffer: u32, format: i32, data: &[u8], frequency: i32);
    /// `alBufferData` from engine memory at `data` (the wave `SampleDataStart`,
    /// valid until `sound_unload_data`).
    fn al_buffer_data_at(
        &mut self,
        buffer: u32,
        format: i32,
        data: usize,
        size: i32,
        frequency: i32,
    );
    fn al_buffer_size(&mut self, buffer: u32) -> i32;
    fn al_source_queue_buffers(&mut self, source: u32, buffers: &[u32]);
    fn al_source_unqueue_buffer(&mut self, source: u32) -> u32;
    fn al_get_source_i(&mut self, source: u32, param: i32) -> i32;
    fn al_source_i(&mut self, source: u32, param: i32, value: i32);
    fn al_source_f(&mut self, source: u32, param: i32, value: f32);
    fn al_source_fv(&mut self, source: u32, param: i32, value: Vec3);
    fn al_source_play(&mut self, source: u32);
    fn al_source_pause(&mut self, source: u32);
    fn al_source_stop(&mut self, source: u32);
    fn al_source_rewind(&mut self, source: u32);
    fn al_listener_fv(&mut self, param: i32, value: &[f32]);
    fn al_distance_model(&mut self, model: i32);
    fn al_doppler_factor(&mut self, factor: f32);
    fn alc_open_device(&mut self) -> usize;
    fn alc_create_context(&mut self, device: usize, attributes: &[i32]) -> usize;
    fn alc_make_context_current(&mut self, context: usize);
    fn alc_destroy_context(&mut self, context: usize);
    fn alc_close_device(&mut self, device: usize);
}
