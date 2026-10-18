//! `extern "C"` exports for the ALAudio port (`hp2_audio`).
//!
//! Rust owns the subsystem logic (`Init`/`Destroy`, registration, playback,
//! stop/modify, `Update`, music, lip sync, the stream state machine, and the
//! Exec commands). C++ keeps the UObject shell, the config properties, the
//! engine services, and the OpenAL trampolines.
//!
//! Engine and OpenAL calls reach Rust through [`FfiHost`], a table of C
//! function pointers filled in by the C++ shell (`ALAudioSubsystem.cpp`). The
//! shell builds a fresh copy per call with `ctx` pointing at its per-call
//! exception slot. [`FfiHost`] implements `hp2_audio::host::Host`, so the Rust
//! logic never sees a C++ type. A trampoline that catches a C++ exception
//! stores it in the slot and returns 0. Rust then returns `Err(HostFailed)`
//! and stops. The shell rethrows the stored exception after the export
//! returns.
//!
//! Export status: 1 means success, 0 means the host failed or a panic was
//! caught. Host function status: 1 means success, 0 means the engine threw.
//!
//! The subsystem is opaque to C++ (`FfiAudioSubsystem`). The shell creates it
//! with `hp2_rs_audio_subsystem_new` and frees it with
//! `hp2_rs_audio_subsystem_free`. Exports are not re-entrant: the shell must
//! not call an export while another export is on the stack. Re-entry from the
//! engine into `RegisterSound` (through `USound::Data.Load`) is stopped by
//! `Subsystem::register_sound`, which checks `sound_is_registered` (the shell
//! calls `USound::IsRegistered()`, as the original does). The `0xDEADBEEF`
//! handle written before the load is the other half of that guard.

use std::ffi::c_void;
use std::panic::AssertUnwindSafe;

use hp2_audio::exec::ExecCommand;
use hp2_audio::host::{
    ActorInfo, Host, HostFailed, HostResult, LogKind, SoundInfo, ViewportInfo, VorbisInfo, WaveInfo,
};
use hp2_audio::playback::PlayRequest;
use hp2_audio::subsystem::{Config, Subsystem};
use hp2_audio::update::UpdateView;
use hp2_audio::vec3::Vec3;
use hp2_ffi_support::guard;

/// Opaque to C++. Always a `hp2_audio::subsystem::Subsystem` underneath.
#[repr(C)]
pub struct FfiAudioSubsystem {
    _opaque: [u8; 0],
}

/// `FVector` as three `f32`s (12 bytes, UE1 layout).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FfiVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

fn vec3_of(v: FfiVec3) -> Vec3 {
    [v.x, v.y, v.z]
}

fn ffi_vec3(v: Vec3) -> FfiVec3 {
    FfiVec3 {
        x: v[0],
        y: v[1],
        z: v[2],
    }
}

/// The `UALAudioSubsystem` config properties the Rust logic reads. Init
/// writes `roll_off` back (`if (RollOff <= 0) RollOff = 1`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiAudioConfig {
    pub doppler_factor: f32,
    pub music_volume: f32,
    pub sound_volume: f32,
    pub roll_off: f32,
    pub max_channels: i32,
    pub max_radius_multiplier: f32,
    pub use_precache: u8,
    pub use_eax: u8,
    pub use_mmsystem: u8,
    pub reverse_stereo: u8,
}

impl From<&FfiAudioConfig> for Config {
    fn from(c: &FfiAudioConfig) -> Self {
        Config {
            doppler_factor: c.doppler_factor,
            music_volume: c.music_volume,
            sound_volume: c.sound_volume,
            roll_off: c.roll_off,
            max_channels: c.max_channels,
            use_precache: c.use_precache != 0,
            max_radius_multiplier: c.max_radius_multiplier,
            use_eax: c.use_eax != 0,
            use_mmsystem: c.use_mmsystem != 0,
            reverse_stereo: c.reverse_stereo != 0,
        }
    }
}

/// `USound` fields for `Host::sound_info`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiSoundInfo {
    pub core_flags: i32,
    pub working_flags: i32,
    pub raw_num_samples: i32,
    pub raw_bits_per_sample: i32,
    pub raw_num_channels: i32,
    pub raw_sample_rate: i32,
    pub is_ogg: u8,
}

impl From<FfiSoundInfo> for SoundInfo {
    fn from(v: FfiSoundInfo) -> Self {
        SoundInfo {
            core_flags: v.core_flags,
            working_flags: v.working_flags,
            is_ogg: v.is_ogg != 0,
            raw_num_samples: v.raw_num_samples,
            raw_bits_per_sample: v.raw_bits_per_sample,
            raw_num_channels: v.raw_num_channels,
            raw_sample_rate: v.raw_sample_rate,
        }
    }
}

/// `FWaveModInfo` after `ReadWaveInfo`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiWaveInfo {
    pub sample_loops_num: i32,
    pub bits_per_sample: i32,
    pub channels: i32,
    pub samples_per_sec: i32,
    pub sample_data: usize,
    pub sample_data_size: i32,
}

impl From<FfiWaveInfo> for WaveInfo {
    fn from(v: FfiWaveInfo) -> Self {
        WaveInfo {
            sample_loops_num: v.sample_loops_num,
            bits_per_sample: v.bits_per_sample,
            channels: v.channels,
            samples_per_sec: v.samples_per_sec,
            sample_data: v.sample_data,
            sample_data_size: v.sample_data_size,
        }
    }
}

/// Ogg decoder report.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiVorbisInfo {
    pub pcm_total: i64,
    pub rate: i32,
    pub channels: i32,
    pub time_total: f64,
}

impl From<FfiVorbisInfo> for VorbisInfo {
    fn from(v: FfiVorbisInfo) -> Self {
        VorbisInfo {
            pcm_total: v.pcm_total,
            rate: v.rate,
            channels: v.channels,
            time_total: v.time_total,
        }
    }
}

/// `AActor` fields the subsystem reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiActorInfo {
    pub location: FfiVec3,
    pub velocity: FfiVec3,
    pub sound_radius: f32,
    pub ambient_sound: usize,
    pub index: i32,
    pub level: usize,
    pub actor_level: usize,
    pub sound_pitch: u8,
    pub sound_volume: u8,
    pub sound_occlusion: u8,
    pub light_is_none: u8,
    pub light_brightness: u8,
}

impl From<FfiActorInfo> for ActorInfo {
    fn from(v: FfiActorInfo) -> Self {
        ActorInfo {
            location: vec3_of(v.location),
            velocity: vec3_of(v.velocity),
            sound_pitch: v.sound_pitch,
            sound_volume: v.sound_volume,
            sound_radius: v.sound_radius,
            sound_occlusion: v.sound_occlusion,
            ambient_sound: v.ambient_sound,
            index: v.index,
            level: v.level,
            actor_level: v.actor_level,
            light_is_none: v.light_is_none != 0,
            light_brightness: v.light_brightness,
        }
    }
}

/// `UViewport` fields, with `ViewTarget` already resolved by the shell.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiViewportInfo {
    pub realtime: u8,
    pub paused: u8,
    pub view_target: usize,
    pub view_target_level: usize,
    pub view_target_actor_level: usize,
    pub viewport_actor_level: usize,
    pub view_target_location: FfiVec3,
}

impl From<FfiViewportInfo> for ViewportInfo {
    fn from(v: FfiViewportInfo) -> Self {
        ViewportInfo {
            realtime: v.realtime != 0,
            paused: v.paused != 0,
            view_target: v.view_target,
            view_target_level: v.view_target_level,
            view_target_actor_level: v.view_target_actor_level,
            viewport_actor_level: v.viewport_actor_level,
            view_target_location: vec3_of(v.view_target_location),
        }
    }
}

/// Out-parameter of `gfile_pop_completed_chunk`. `has == 0` means no chunk.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiPopCompleted {
    pub has: u8,
    pub dest: usize,
    pub terminal: u8,
}

/// `UALAudioSubsystem::PlaySound` arguments.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiPlayRequest {
    pub actor: usize,
    pub id: i32,
    pub sound: usize,
    pub location: FfiVec3,
    pub volume: f32,
    pub radius: f32,
    pub pitch: f32,
    pub flags: i32,
    pub fade_duration: f32,
    pub in_priority: f32,
}

impl From<&FfiPlayRequest> for PlayRequest {
    fn from(v: &FfiPlayRequest) -> Self {
        PlayRequest {
            actor: v.actor,
            id: v.id,
            sound: v.sound,
            location: vec3_of(v.location),
            volume: v.volume,
            radius: v.radius,
            pitch: v.pitch,
            flags: v.flags,
            fade_duration: v.fade_duration,
            in_priority: v.in_priority,
        }
    }
}

/// `UALAudioSubsystem::Update` inputs from `Viewport->Actor` and `SceneNode`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FfiUpdateView {
    pub view_location: FfiVec3,
    pub x_axis: FfiVec3,
    pub y_axis: FfiVec3,
    pub z_axis: FfiVec3,
}

/// Exec commands that Rust executes. C++ parses the command text first.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiExecCommand {
    PauseSounds = 0,
    UnpauseSounds = 1,
    RestartSounds = 2,
    PlayingSoundStats = 3,
}

/// Maps a raw `hp2_rs_audio_exec` command code to an [`ExecCommand`]. The C++
/// shell passes a `u32`, not the enum, so an unknown code is rejected here
/// instead of being undefined behavior.
fn exec_command(cmd: u32) -> Option<ExecCommand> {
    const PAUSE: u32 = FfiExecCommand::PauseSounds as u32;
    const UNPAUSE: u32 = FfiExecCommand::UnpauseSounds as u32;
    const RESTART: u32 = FfiExecCommand::RestartSounds as u32;
    const STATS: u32 = FfiExecCommand::PlayingSoundStats as u32;
    Some(match cmd {
        PAUSE => ExecCommand::PauseSounds,
        UNPAUSE => ExecCommand::UnpauseSounds,
        RESTART => ExecCommand::RestartSounds,
        STATS => ExecCommand::PlayingSoundStats,
        _ => return None,
    })
}

/// The engine and OpenAL trampolines. Field order is part of the ABI: the
/// C++ shell fills the fields in exactly this order (`kHostTable`).
///
/// Conventions: every function returns `u8`, where 1 means success and 0
/// means the engine threw. Results come back through out-pointers. Strings
/// are `(ptr, len)` UTF-8. The `*_fill` functions write exactly `cap`
/// entries. Strings returned by the host (`sound_name` and friends) point
/// into a shell buffer valid only until the next host call. Rust copies them
/// at once. OpenAL and ALC functions cannot throw and return plain values.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiHost {
    pub ctx: *mut c_void,

    // Logging and time. `kind` is the LogKind code (see `log_kind_code`).
    pub log: unsafe extern "C" fn(ctx: *mut c_void, kind: u32, text: *const u8, len: usize) -> u8,
    pub output: unsafe extern "C" fn(ctx: *mut c_void, text: *const u8, len: usize) -> u8,
    pub app_seconds: unsafe extern "C" fn(ctx: *mut c_void, out: *mut i64) -> u8,
    pub app_error: unsafe extern "C" fn(ctx: *mut c_void, text: *const u8, len: usize) -> u8,

    // USound.
    pub sound_info:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, out: *mut FfiSoundInfo) -> u8,
    pub sound_data_len: unsafe extern "C" fn(ctx: *mut c_void, sound: usize, out: *mut i32) -> u8,
    pub sound_name: unsafe extern "C" fn(
        ctx: *mut c_void,
        sound: usize,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> u8,
    pub sound_path_name: unsafe extern "C" fn(
        ctx: *mut c_void,
        sound: usize,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> u8,
    pub sound_filename: unsafe extern "C" fn(
        ctx: *mut c_void,
        sound: usize,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> u8,
    pub sound_set_working_flags:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, flags: i32) -> u8,
    pub sound_set_handle: unsafe extern "C" fn(ctx: *mut c_void, sound: usize, handle: i32) -> u8,
    pub sound_get_handle:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, id: i32, out: *mut i32) -> u8,
    pub sound_set_duration:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, duration: f32) -> u8,
    pub sound_add_ref:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, delta: i32, out: *mut i32) -> u8,
    pub sound_is_registered:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, out: *mut u8) -> u8,
    pub sound_add_stream_handle:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, id: i32) -> u8,
    pub sound_delete_stream_handle:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, handle: i32) -> u8,
    pub sound_bind_stream: unsafe extern "C" fn(ctx: *mut c_void, sound: usize, id: i32) -> u8,
    pub sound_unbind_stream: unsafe extern "C" fn(ctx: *mut c_void, sound: usize, id: i32) -> u8,
    pub sound_unbind_all_streams: unsafe extern "C" fn(ctx: *mut c_void, sound: usize) -> u8,
    pub sound_delete_all_stream_handles: unsafe extern "C" fn(ctx: *mut c_void, sound: usize) -> u8,
    pub sound_playing_stream_count:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, out: *mut usize) -> u8,
    pub sound_playing_stream_fill:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, out: *mut i32, cap: usize) -> u8,
    pub sound_load_data: unsafe extern "C" fn(ctx: *mut c_void, sound: usize) -> u8,
    pub sound_unload_data: unsafe extern "C" fn(ctx: *mut c_void, sound: usize) -> u8,
    pub sound_wave_info:
        unsafe extern "C" fn(ctx: *mut c_void, sound: usize, out: *mut FfiWaveInfo) -> u8,
    pub sound_count: unsafe extern "C" fn(ctx: *mut c_void, out: *mut usize) -> u8,
    pub sound_fill: unsafe extern "C" fn(ctx: *mut c_void, out: *mut usize, cap: usize) -> u8,
    pub music_sound_new: unsafe extern "C" fn(
        ctx: *mut c_void,
        filename: *const u8,
        len: usize,
        out: *mut usize,
    ) -> u8,

    // Ogg decoder.
    pub vorbis_new: unsafe extern "C" fn(ctx: *mut c_void, out: *mut usize) -> u8,
    pub vorbis_free: unsafe extern "C" fn(ctx: *mut c_void, decoder: usize) -> u8,
    pub vorbis_info:
        unsafe extern "C" fn(ctx: *mut c_void, decoder: usize, out: *mut FfiVorbisInfo) -> u8,

    // GFileStream.
    pub gfile_create_ogg_stream: unsafe extern "C" fn(
        ctx: *mut c_void,
        sound: usize,
        chunk: i32,
        num: i32,
        prime: *mut u8,
        looping: u8,
        decoder: usize,
        out: *mut i32,
    ) -> u8,
    pub gfile_create_xa_stream: unsafe extern "C" fn(
        ctx: *mut c_void,
        sound: usize,
        num_samples: i32,
        chunk: i32,
        num: i32,
        prime: *mut u8,
        looping: u8,
        out: *mut i32,
    ) -> u8,
    pub gfile_destroy_stream: unsafe extern "C" fn(ctx: *mut c_void, id: i32) -> u8,
    pub gfile_request_chunk:
        unsafe extern "C" fn(ctx: *mut c_void, id: i32, dest: usize, out: *mut u8) -> u8,
    pub gfile_pop_completed_chunk:
        unsafe extern "C" fn(ctx: *mut c_void, id: i32, out: *mut FfiPopCompleted) -> u8,

    // Actors, levels, viewports.
    pub actor_info:
        unsafe extern "C" fn(ctx: *mut c_void, actor: usize, out: *mut FfiActorInfo) -> u8,
    pub actor_is_owned_by:
        unsafe extern "C" fn(ctx: *mut c_void, actor: usize, owner: usize, out: *mut u8) -> u8,
    pub level_is_audible_at: unsafe extern "C" fn(
        ctx: *mut c_void,
        level: usize,
        sound_location: FfiVec3,
        listener_location: FfiVec3,
        actor: usize,
        occlusion: u8,
        out: *mut u8,
    ) -> u8,
    pub level_point_region_zone: unsafe extern "C" fn(
        ctx: *mut c_void,
        level: usize,
        actor_level: usize,
        location: FfiVec3,
        out: *mut i32,
    ) -> u8,
    pub level_radius_multiplier: unsafe extern "C" fn(
        ctx: *mut c_void,
        level: usize,
        zone_a: i32,
        zone_b: i32,
        out: *mut f32,
    ) -> u8,
    pub viewport_info:
        unsafe extern "C" fn(ctx: *mut c_void, viewport: usize, out: *mut FfiViewportInfo) -> u8,
    pub level_actor_count:
        unsafe extern "C" fn(ctx: *mut c_void, level: usize, out: *mut usize) -> u8,
    pub level_actor_fill:
        unsafe extern "C" fn(ctx: *mut c_void, level: usize, out: *mut usize, cap: usize) -> u8,

    // Lip sync (UAnimChannel). A channel of 0 means "none".
    pub lipsync_start: unsafe extern "C" fn(ctx: *mut c_void, actor: usize, out: *mut usize) -> u8,
    pub lipsync_set_frame:
        unsafe extern "C" fn(ctx: *mut c_void, channel: usize, sound: usize, elapsed: f32) -> u8,
    pub lipsync_release: unsafe extern "C" fn(ctx: *mut c_void, channel: usize) -> u8,

    // OpenAL and ALC. These cannot throw.
    pub al_get_error: unsafe extern "C" fn(ctx: *mut c_void) -> i32,
    pub al_gen_source: unsafe extern "C" fn(ctx: *mut c_void) -> u32,
    pub al_delete_source: unsafe extern "C" fn(ctx: *mut c_void, source: u32),
    pub al_gen_buffers: unsafe extern "C" fn(ctx: *mut c_void, out: *mut u32, n: usize),
    pub al_delete_buffers: unsafe extern "C" fn(ctx: *mut c_void, buffers: *const u32, n: usize),
    pub al_buffer_data: unsafe extern "C" fn(
        ctx: *mut c_void,
        buffer: u32,
        format: i32,
        data: *const u8,
        len: usize,
        frequency: i32,
    ),
    pub al_buffer_data_at: unsafe extern "C" fn(
        ctx: *mut c_void,
        buffer: u32,
        format: i32,
        data: usize,
        size: i32,
        frequency: i32,
    ),
    pub al_buffer_size: unsafe extern "C" fn(ctx: *mut c_void, buffer: u32) -> i32,
    pub al_source_queue_buffers:
        unsafe extern "C" fn(ctx: *mut c_void, source: u32, buffers: *const u32, n: usize),
    pub al_source_unqueue_buffer: unsafe extern "C" fn(ctx: *mut c_void, source: u32) -> u32,
    pub al_get_source_i: unsafe extern "C" fn(ctx: *mut c_void, source: u32, param: i32) -> i32,
    pub al_source_i: unsafe extern "C" fn(ctx: *mut c_void, source: u32, param: i32, value: i32),
    pub al_source_f: unsafe extern "C" fn(ctx: *mut c_void, source: u32, param: i32, value: f32),
    pub al_source_fv:
        unsafe extern "C" fn(ctx: *mut c_void, source: u32, param: i32, value: FfiVec3),
    pub al_source_play: unsafe extern "C" fn(ctx: *mut c_void, source: u32),
    pub al_source_pause: unsafe extern "C" fn(ctx: *mut c_void, source: u32),
    pub al_source_stop: unsafe extern "C" fn(ctx: *mut c_void, source: u32),
    pub al_source_rewind: unsafe extern "C" fn(ctx: *mut c_void, source: u32),
    pub al_listener_fv: unsafe extern "C" fn(ctx: *mut c_void, param: i32, value: *const f32),
    pub al_distance_model: unsafe extern "C" fn(ctx: *mut c_void, model: i32),
    pub al_doppler_factor: unsafe extern "C" fn(ctx: *mut c_void, factor: f32),
    pub alc_open_device: unsafe extern "C" fn(ctx: *mut c_void) -> usize,
    /// `attributes` is NUL-terminated; `alcCreateContext` reads until the `0`.
    pub alc_create_context:
        unsafe extern "C" fn(ctx: *mut c_void, device: usize, attributes: *const i32) -> usize,
    pub alc_make_context_current: unsafe extern "C" fn(ctx: *mut c_void, context: usize),
    pub alc_destroy_context: unsafe extern "C" fn(ctx: *mut c_void, context: usize),
    pub alc_close_device: unsafe extern "C" fn(ctx: *mut c_void, device: usize),
}

/// `LogKind` as the shell's `NAME_*` category index.
fn log_kind_code(kind: LogKind) -> u32 {
    match kind {
        LogKind::Init => 0,
        LogKind::Exit => 1,
        LogKind::DevSound => 2,
        LogKind::DevMusic => 3,
        LogKind::Warning => 4,
        LogKind::Log => 5,
    }
}

fn status(ok: u8) -> HostResult<()> {
    if ok != 0 { Ok(()) } else { Err(HostFailed) }
}

/// Runs a host call that writes a `(ptr, len)` string, and copies it out.
fn string_out(call: impl FnOnce(*mut *const u8, *mut usize) -> u8) -> HostResult<String> {
    let mut ptr: *const u8 = std::ptr::null();
    let mut len: usize = 0;
    status(call(&mut ptr, &mut len))?;
    if len == 0 {
        return Ok(String::new());
    }
    // SAFETY: the shell reports a (ptr, len) pair valid until its next host
    // call. It is copied here, before any other host call.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

impl Host for FfiHost {
    fn log(&mut self, kind: LogKind, text: &str) -> HostResult<()> {
        status(unsafe { (self.log)(self.ctx, log_kind_code(kind), text.as_ptr(), text.len()) })
    }

    fn output(&mut self, text: &str) -> HostResult<()> {
        status(unsafe { (self.output)(self.ctx, text.as_ptr(), text.len()) })
    }

    fn app_seconds(&mut self) -> HostResult<i64> {
        let mut out = 0i64;
        status(unsafe { (self.app_seconds)(self.ctx, &mut out) })?;
        Ok(out)
    }

    fn app_error(&mut self, text: &str) -> HostResult<()> {
        status(unsafe { (self.app_error)(self.ctx, text.as_ptr(), text.len()) })
    }

    fn sound_info(&mut self, sound: usize) -> HostResult<SoundInfo> {
        let mut out = FfiSoundInfo::default();
        status(unsafe { (self.sound_info)(self.ctx, sound, &mut out) })?;
        Ok(out.into())
    }

    fn sound_data_len(&mut self, sound: usize) -> HostResult<i32> {
        let mut out = 0i32;
        status(unsafe { (self.sound_data_len)(self.ctx, sound, &mut out) })?;
        Ok(out)
    }

    fn sound_name(&mut self, sound: usize) -> HostResult<String> {
        let f = self.sound_name;
        let ctx = self.ctx;
        string_out(|p, l| unsafe { f(ctx, sound, p, l) })
    }

    fn sound_path_name(&mut self, sound: usize) -> HostResult<String> {
        let f = self.sound_path_name;
        let ctx = self.ctx;
        string_out(|p, l| unsafe { f(ctx, sound, p, l) })
    }

    fn sound_filename(&mut self, sound: usize) -> HostResult<String> {
        let f = self.sound_filename;
        let ctx = self.ctx;
        string_out(|p, l| unsafe { f(ctx, sound, p, l) })
    }

    fn sound_set_working_flags(&mut self, sound: usize, flags: i32) -> HostResult<()> {
        status(unsafe { (self.sound_set_working_flags)(self.ctx, sound, flags) })
    }

    fn sound_set_handle(&mut self, sound: usize, handle: i32) -> HostResult<()> {
        status(unsafe { (self.sound_set_handle)(self.ctx, sound, handle) })
    }

    fn sound_get_handle(&mut self, sound: usize, id: i32) -> HostResult<i32> {
        let mut out = 0i32;
        status(unsafe { (self.sound_get_handle)(self.ctx, sound, id, &mut out) })?;
        Ok(out)
    }

    fn sound_set_duration(&mut self, sound: usize, duration: f32) -> HostResult<()> {
        status(unsafe { (self.sound_set_duration)(self.ctx, sound, duration) })
    }

    fn sound_add_ref(&mut self, sound: usize, delta: i32) -> HostResult<i32> {
        let mut out = 0i32;
        status(unsafe { (self.sound_add_ref)(self.ctx, sound, delta, &mut out) })?;
        Ok(out)
    }

    fn sound_is_registered(&mut self, sound: usize) -> HostResult<bool> {
        let mut out = 0u8;
        status(unsafe { (self.sound_is_registered)(self.ctx, sound, &mut out) })?;
        Ok(out != 0)
    }

    fn sound_add_stream_handle(&mut self, sound: usize, id: i32) -> HostResult<()> {
        status(unsafe { (self.sound_add_stream_handle)(self.ctx, sound, id) })
    }

    fn sound_delete_stream_handle(&mut self, sound: usize, handle: i32) -> HostResult<()> {
        status(unsafe { (self.sound_delete_stream_handle)(self.ctx, sound, handle) })
    }

    fn sound_bind_stream(&mut self, sound: usize, id: i32) -> HostResult<()> {
        status(unsafe { (self.sound_bind_stream)(self.ctx, sound, id) })
    }

    fn sound_unbind_stream(&mut self, sound: usize, id: i32) -> HostResult<()> {
        status(unsafe { (self.sound_unbind_stream)(self.ctx, sound, id) })
    }

    fn sound_unbind_all_streams(&mut self, sound: usize) -> HostResult<()> {
        status(unsafe { (self.sound_unbind_all_streams)(self.ctx, sound) })
    }

    fn sound_delete_all_stream_handles(&mut self, sound: usize) -> HostResult<()> {
        status(unsafe { (self.sound_delete_all_stream_handles)(self.ctx, sound) })
    }

    fn sound_playing_stream_count(&mut self, sound: usize) -> HostResult<usize> {
        let mut out = 0usize;
        status(unsafe { (self.sound_playing_stream_count)(self.ctx, sound, &mut out) })?;
        Ok(out)
    }

    fn sound_playing_stream_fill(&mut self, sound: usize, out: &mut [i32]) -> HostResult<()> {
        status(unsafe {
            (self.sound_playing_stream_fill)(self.ctx, sound, out.as_mut_ptr(), out.len())
        })
    }

    fn sound_load_data(&mut self, sound: usize) -> HostResult<()> {
        status(unsafe { (self.sound_load_data)(self.ctx, sound) })
    }

    fn sound_unload_data(&mut self, sound: usize) -> HostResult<()> {
        status(unsafe { (self.sound_unload_data)(self.ctx, sound) })
    }

    fn sound_wave_info(&mut self, sound: usize) -> HostResult<WaveInfo> {
        let mut out = FfiWaveInfo::default();
        status(unsafe { (self.sound_wave_info)(self.ctx, sound, &mut out) })?;
        Ok(out.into())
    }

    fn sound_count(&mut self) -> HostResult<usize> {
        let mut out = 0usize;
        status(unsafe { (self.sound_count)(self.ctx, &mut out) })?;
        Ok(out)
    }

    fn sound_fill(&mut self, out: &mut [usize]) -> HostResult<()> {
        status(unsafe { (self.sound_fill)(self.ctx, out.as_mut_ptr(), out.len()) })
    }

    fn music_sound_new(&mut self, filename: &str) -> HostResult<usize> {
        let mut out = 0usize;
        status(unsafe {
            (self.music_sound_new)(self.ctx, filename.as_ptr(), filename.len(), &mut out)
        })?;
        Ok(out)
    }

    fn vorbis_new(&mut self) -> HostResult<usize> {
        let mut out = 0usize;
        status(unsafe { (self.vorbis_new)(self.ctx, &mut out) })?;
        Ok(out)
    }

    fn vorbis_free(&mut self, decoder: usize) -> HostResult<()> {
        status(unsafe { (self.vorbis_free)(self.ctx, decoder) })
    }

    fn vorbis_info(&mut self, decoder: usize) -> HostResult<VorbisInfo> {
        let mut out = FfiVorbisInfo::default();
        status(unsafe { (self.vorbis_info)(self.ctx, decoder, &mut out) })?;
        Ok(out.into())
    }

    fn gfile_create_ogg_stream(
        &mut self,
        sound: usize,
        chunk: i32,
        num: i32,
        prime: &mut [u8],
        looping: bool,
        decoder: usize,
    ) -> HostResult<i32> {
        let mut out = 0i32;
        status(unsafe {
            (self.gfile_create_ogg_stream)(
                self.ctx,
                sound,
                chunk,
                num,
                prime.as_mut_ptr(),
                looping as u8,
                decoder,
                &mut out,
            )
        })?;
        Ok(out)
    }

    fn gfile_create_xa_stream(
        &mut self,
        sound: usize,
        num_samples: i32,
        chunk: i32,
        num: i32,
        prime: &mut [u8],
        looping: bool,
    ) -> HostResult<i32> {
        let mut out = 0i32;
        status(unsafe {
            (self.gfile_create_xa_stream)(
                self.ctx,
                sound,
                num_samples,
                chunk,
                num,
                prime.as_mut_ptr(),
                looping as u8,
                &mut out,
            )
        })?;
        Ok(out)
    }

    fn gfile_destroy_stream(&mut self, id: i32) -> HostResult<()> {
        status(unsafe { (self.gfile_destroy_stream)(self.ctx, id) })
    }

    fn gfile_request_chunk(&mut self, id: i32, dest: usize) -> HostResult<bool> {
        let mut out = 0u8;
        status(unsafe { (self.gfile_request_chunk)(self.ctx, id, dest, &mut out) })?;
        Ok(out != 0)
    }

    fn gfile_pop_completed_chunk(&mut self, id: i32) -> HostResult<Option<(usize, bool)>> {
        let mut out = FfiPopCompleted::default();
        status(unsafe { (self.gfile_pop_completed_chunk)(self.ctx, id, &mut out) })?;
        Ok((out.has != 0).then_some((out.dest, out.terminal != 0)))
    }

    fn actor_info(&mut self, actor: usize) -> HostResult<ActorInfo> {
        let mut out = FfiActorInfo::default();
        status(unsafe { (self.actor_info)(self.ctx, actor, &mut out) })?;
        Ok(out.into())
    }

    fn actor_is_owned_by(&mut self, actor: usize, owner: usize) -> HostResult<bool> {
        let mut out = 0u8;
        status(unsafe { (self.actor_is_owned_by)(self.ctx, actor, owner, &mut out) })?;
        Ok(out != 0)
    }

    fn level_is_audible_at(
        &mut self,
        level: usize,
        sound_location: Vec3,
        listener_location: Vec3,
        actor: usize,
        occlusion: u8,
    ) -> HostResult<bool> {
        let mut out = 0u8;
        status(unsafe {
            (self.level_is_audible_at)(
                self.ctx,
                level,
                ffi_vec3(sound_location),
                ffi_vec3(listener_location),
                actor,
                occlusion,
                &mut out,
            )
        })?;
        Ok(out != 0)
    }

    fn level_point_region_zone(
        &mut self,
        level: usize,
        actor_level: usize,
        location: Vec3,
    ) -> HostResult<i32> {
        let mut out = 0i32;
        status(unsafe {
            (self.level_point_region_zone)(
                self.ctx,
                level,
                actor_level,
                ffi_vec3(location),
                &mut out,
            )
        })?;
        Ok(out)
    }

    fn level_radius_multiplier(
        &mut self,
        level: usize,
        zone_a: i32,
        zone_b: i32,
    ) -> HostResult<f32> {
        let mut out = 0f32;
        status(unsafe {
            (self.level_radius_multiplier)(self.ctx, level, zone_a, zone_b, &mut out)
        })?;
        Ok(out)
    }

    fn viewport_info(&mut self, viewport: usize) -> HostResult<ViewportInfo> {
        let mut out = FfiViewportInfo::default();
        status(unsafe { (self.viewport_info)(self.ctx, viewport, &mut out) })?;
        Ok(out.into())
    }

    fn level_actor_count(&mut self, level: usize) -> HostResult<usize> {
        let mut out = 0usize;
        status(unsafe { (self.level_actor_count)(self.ctx, level, &mut out) })?;
        Ok(out)
    }

    fn level_actor_fill(&mut self, level: usize, out: &mut [usize]) -> HostResult<()> {
        status(unsafe { (self.level_actor_fill)(self.ctx, level, out.as_mut_ptr(), out.len()) })
    }

    fn lipsync_start(&mut self, actor: usize) -> HostResult<usize> {
        let mut out = 0usize;
        status(unsafe { (self.lipsync_start)(self.ctx, actor, &mut out) })?;
        Ok(out)
    }

    fn lipsync_set_frame(&mut self, channel: usize, sound: usize, elapsed: f32) -> HostResult<()> {
        status(unsafe { (self.lipsync_set_frame)(self.ctx, channel, sound, elapsed) })
    }

    fn lipsync_release(&mut self, channel: usize) -> HostResult<()> {
        status(unsafe { (self.lipsync_release)(self.ctx, channel) })
    }

    fn al_get_error(&mut self) -> i32 {
        unsafe { (self.al_get_error)(self.ctx) }
    }

    fn al_gen_source(&mut self) -> u32 {
        unsafe { (self.al_gen_source)(self.ctx) }
    }

    fn al_delete_source(&mut self, source: u32) {
        unsafe { (self.al_delete_source)(self.ctx, source) }
    }

    fn al_gen_buffers(&mut self, out: &mut [u32]) {
        unsafe { (self.al_gen_buffers)(self.ctx, out.as_mut_ptr(), out.len()) }
    }

    fn al_delete_buffers(&mut self, buffers: &[u32]) {
        unsafe { (self.al_delete_buffers)(self.ctx, buffers.as_ptr(), buffers.len()) }
    }

    fn al_buffer_data(&mut self, buffer: u32, format: i32, data: &[u8], frequency: i32) {
        unsafe {
            (self.al_buffer_data)(
                self.ctx,
                buffer,
                format,
                data.as_ptr(),
                data.len(),
                frequency,
            )
        }
    }

    fn al_buffer_data_at(
        &mut self,
        buffer: u32,
        format: i32,
        data: usize,
        size: i32,
        frequency: i32,
    ) {
        unsafe { (self.al_buffer_data_at)(self.ctx, buffer, format, data, size, frequency) }
    }

    fn al_buffer_size(&mut self, buffer: u32) -> i32 {
        unsafe { (self.al_buffer_size)(self.ctx, buffer) }
    }

    fn al_source_queue_buffers(&mut self, source: u32, buffers: &[u32]) {
        unsafe { (self.al_source_queue_buffers)(self.ctx, source, buffers.as_ptr(), buffers.len()) }
    }

    fn al_source_unqueue_buffer(&mut self, source: u32) -> u32 {
        unsafe { (self.al_source_unqueue_buffer)(self.ctx, source) }
    }

    fn al_get_source_i(&mut self, source: u32, param: i32) -> i32 {
        unsafe { (self.al_get_source_i)(self.ctx, source, param) }
    }

    fn al_source_i(&mut self, source: u32, param: i32, value: i32) {
        unsafe { (self.al_source_i)(self.ctx, source, param, value) }
    }

    fn al_source_f(&mut self, source: u32, param: i32, value: f32) {
        unsafe { (self.al_source_f)(self.ctx, source, param, value) }
    }

    fn al_source_fv(&mut self, source: u32, param: i32, value: Vec3) {
        unsafe { (self.al_source_fv)(self.ctx, source, param, ffi_vec3(value)) }
    }

    fn al_source_play(&mut self, source: u32) {
        unsafe { (self.al_source_play)(self.ctx, source) }
    }

    fn al_source_pause(&mut self, source: u32) {
        unsafe { (self.al_source_pause)(self.ctx, source) }
    }

    fn al_source_stop(&mut self, source: u32) {
        unsafe { (self.al_source_stop)(self.ctx, source) }
    }

    fn al_source_rewind(&mut self, source: u32) {
        unsafe { (self.al_source_rewind)(self.ctx, source) }
    }

    fn al_listener_fv(&mut self, param: i32, value: &[f32]) {
        unsafe { (self.al_listener_fv)(self.ctx, param, value.as_ptr()) }
    }

    fn al_distance_model(&mut self, model: i32) {
        unsafe { (self.al_distance_model)(self.ctx, model) }
    }

    fn al_doppler_factor(&mut self, factor: f32) {
        unsafe { (self.al_doppler_factor)(self.ctx, factor) }
    }

    fn alc_open_device(&mut self) -> usize {
        unsafe { (self.alc_open_device)(self.ctx) }
    }

    fn alc_create_context(&mut self, device: usize, attributes: &[i32]) -> usize {
        debug_assert_eq!(attributes.last(), Some(&0));
        unsafe { (self.alc_create_context)(self.ctx, device, attributes.as_ptr()) }
    }

    fn alc_make_context_current(&mut self, context: usize) {
        unsafe { (self.alc_make_context_current)(self.ctx, context) }
    }

    fn alc_destroy_context(&mut self, context: usize) {
        unsafe { (self.alc_destroy_context)(self.ctx, context) }
    }

    fn alc_close_device(&mut self, device: usize) {
        unsafe { (self.alc_close_device)(self.ctx, device) }
    }
}

fn status_of(result: HostResult<()>) -> i32 {
    match result {
        Ok(()) => 1,
        Err(HostFailed) => 0,
    }
}

/// # Safety
/// `sub` must be null or a pointer from `hp2_rs_audio_subsystem_new` that has
/// not been freed.
unsafe fn sub_mut<'a>(sub: *mut FfiAudioSubsystem) -> Option<&'a mut Subsystem> {
    unsafe { sub.cast::<Subsystem>().as_mut() }
}

/// # Safety
/// `host` must be null or point to a valid `FfiHost` for the call's duration.
unsafe fn host_copy(host: *const FfiHost) -> Option<FfiHost> {
    unsafe { host.as_ref().copied() }
}

/// # Safety
/// `ptr` must be null or valid for `len` bytes.
unsafe fn text_arg(ptr: *const u8, len: usize) -> String {
    if ptr.is_null() || len == 0 {
        return String::new();
    }
    unsafe { String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned() }
}

/// Creates the opaque subsystem. `last_time` is `appSeconds()` in FTime ticks,
/// stored as `LastTime` by the UObject constructor.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_audio_subsystem_new(last_time: i64) -> *mut FfiAudioSubsystem {
    guard(
        "hp2_rs_audio_subsystem_new",
        std::ptr::null_mut(),
        AssertUnwindSafe(move || {
            Box::into_raw(Box::new(Subsystem::new(last_time))).cast::<FfiAudioSubsystem>()
        }),
    )
}

/// Frees a subsystem from `hp2_rs_audio_subsystem_new`.
///
/// # Safety
/// `sub` must be null or a pointer from `hp2_rs_audio_subsystem_new` that has
/// not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_subsystem_free(sub: *mut FfiAudioSubsystem) {
    guard(
        "hp2_rs_audio_subsystem_free",
        (),
        AssertUnwindSafe(move || unsafe {
            if !sub.is_null() {
                drop(Box::from_raw(sub.cast::<Subsystem>()));
            }
        }),
    )
}

/// `UALAudioSubsystem::Init`. Writes `*out_ok` (1 or 0) and `cfg->roll_off`.
///
/// # Safety
/// All pointers must be valid for the call as described on `FfiAudioSubsystem`
/// and `FfiHost`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_init(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cfg: *mut FfiAudioConfig,
    out_ok: *mut u8,
) -> i32 {
    guard(
        "hp2_rs_audio_init",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cfg), Some(out_ok)) =
                (sub_mut(sub), host_copy(host), cfg.as_mut(), out_ok.as_mut())
            else {
                return 0;
            };
            let mut config = Config::from(&*cfg);
            let result = sub.init(&mut host, &mut config);
            cfg.roll_off = config.roll_off;
            match result {
                Ok(ok) => {
                    *out_ok = ok as u8;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::Destroy` teardown.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_destroy(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cfg: *const FfiAudioConfig,
) -> i32 {
    guard(
        "hp2_rs_audio_destroy",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cfg)) =
                (sub_mut(sub), host_copy(host), cfg.as_ref())
            else {
                return 0;
            };
            status_of(sub.destroy(&mut host, &Config::from(cfg)))
        }),
    )
}

/// `UALAudioSubsystem::ShutdownAfterError`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_shutdown_after_error(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
) -> i32 {
    guard(
        "hp2_rs_audio_shutdown_after_error",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host)) = (sub_mut(sub), host_copy(host)) else {
                return 0;
            };
            status_of(sub.shutdown_after_error(&mut host))
        }),
    )
}

/// `UALAudioSubsystem::SetViewport`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_set_viewport(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cfg: *const FfiAudioConfig,
    viewport: usize,
) -> i32 {
    guard(
        "hp2_rs_audio_set_viewport",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cfg)) =
                (sub_mut(sub), host_copy(host), cfg.as_ref())
            else {
                return 0;
            };
            status_of(sub.set_viewport(&mut host, &Config::from(cfg), viewport))
        }),
    )
}

/// `UALAudioSubsystem::RegisterSound`. Writes the returned bool to `*out`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_register_sound(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    sound: usize,
    out: *mut u8,
) -> i32 {
    guard(
        "hp2_rs_audio_register_sound",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(out)) =
                (sub_mut(sub), host_copy(host), out.as_mut())
            else {
                return 0;
            };
            match sub.register_sound(&mut host, sound) {
                Ok(ok) => {
                    *out = ok as u8;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::UnregisterSound`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_unregister_sound(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    sound: usize,
    id: i32,
) -> i32 {
    guard(
        "hp2_rs_audio_unregister_sound",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host)) = (sub_mut(sub), host_copy(host)) else {
                return 0;
            };
            status_of(sub.unregister_sound(&mut host, sound, id))
        }),
    )
}

/// `UALAudioSubsystem::PlaySound`. Writes the source index (or -1) to `*out`.
///
/// # Safety
/// See `hp2_rs_audio_init`. `req` must be valid for reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_play_sound(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cfg: *const FfiAudioConfig,
    req: *const FfiPlayRequest,
    out: *mut i32,
) -> i32 {
    guard(
        "hp2_rs_audio_play_sound",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cfg), Some(req), Some(out)) = (
                sub_mut(sub),
                host_copy(host),
                cfg.as_ref(),
                req.as_ref(),
                out.as_mut(),
            ) else {
                return 0;
            };
            match sub.play_sound(&mut host, &Config::from(cfg), &PlayRequest::from(req)) {
                Ok(index) => {
                    *out = index;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::StopSound(AActor*, USound*)`. Writes whether any voice stopped.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_stop_sound_by_actor(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    actor: usize,
    sound: usize,
    out: *mut u8,
) -> i32 {
    guard(
        "hp2_rs_audio_stop_sound_by_actor",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(out)) =
                (sub_mut(sub), host_copy(host), out.as_mut())
            else {
                return 0;
            };
            match sub.stop_sound_by_actor(&mut host, actor, sound) {
                Ok(stopped) => {
                    *out = stopped as u8;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::StopSound(AActor*, INT Id, USound*, FLOAT fFadeOutTime)`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_stop_sound_by_id(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    actor: usize,
    id: i32,
    sound: usize,
    fade_out_time: f32,
    out: *mut u8,
) -> i32 {
    guard(
        "hp2_rs_audio_stop_sound_by_id",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(out)) =
                (sub_mut(sub), host_copy(host), out.as_mut())
            else {
                return 0;
            };
            match sub.stop_sound_by_id(&mut host, actor, id, sound, fade_out_time) {
                Ok(stopped) => {
                    *out = stopped as u8;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::ModifySound`. Writes the return value to `*out`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_modify_sound(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    actor: usize,
    id: i32,
    sound: usize,
    parameter: u8,
    value: f32,
    out: *mut u8,
) -> i32 {
    guard(
        "hp2_rs_audio_modify_sound",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(out)) =
                (sub_mut(sub), host_copy(host), out.as_mut())
            else {
                return 0;
            };
            match sub.modify_sound(&mut host, actor, id, sound, parameter, value) {
                Ok(ret) => {
                    *out = ret as u8;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::NoteDestroy`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_note_destroy(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    actor: usize,
) -> i32 {
    guard(
        "hp2_rs_audio_note_destroy",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host)) = (sub_mut(sub), host_copy(host)) else {
                return 0;
            };
            status_of(sub.note_destroy(&mut host, actor))
        }),
    )
}

/// `UALAudioSubsystem::Update`.
///
/// # Safety
/// See `hp2_rs_audio_init`. `view` must be valid for reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_update(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cfg: *const FfiAudioConfig,
    view: *const FfiUpdateView,
) -> i32 {
    guard(
        "hp2_rs_audio_update",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cfg), Some(view)) =
                (sub_mut(sub), host_copy(host), cfg.as_ref(), view.as_ref())
            else {
                return 0;
            };
            let view = UpdateView {
                view_location: vec3_of(view.view_location),
                x_axis: vec3_of(view.x_axis),
                y_axis: vec3_of(view.y_axis),
                z_axis: vec3_of(view.z_axis),
            };
            status_of(sub.update(&mut host, &Config::from(cfg), &view))
        }),
    )
}

/// `UALAudioSubsystem::Exec` for the commands Rust owns.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_exec(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cmd: u32,
) -> i32 {
    guard(
        "hp2_rs_audio_exec",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cmd)) =
                (sub_mut(sub), host_copy(host), exec_command(cmd))
            else {
                return 0;
            };
            status_of(sub.exec(&mut host, cmd))
        }),
    )
}

/// `UALAudioSubsystem::PlayMusic`. Writes the one-based song handle (or 0).
///
/// # Safety
/// See `hp2_rs_audio_init`. `song` must be valid for `song_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_play_music(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    cfg: *const FfiAudioConfig,
    song: *const u8,
    song_len: usize,
    fade_in_time: f32,
    out: *mut i32,
) -> i32 {
    guard(
        "hp2_rs_audio_play_music",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(cfg), Some(out)) =
                (sub_mut(sub), host_copy(host), cfg.as_ref(), out.as_mut())
            else {
                return 0;
            };
            let song = text_arg(song, song_len);
            match sub.play_music(&mut host, &Config::from(cfg), &song, fade_in_time) {
                Ok(handle) => {
                    *out = handle;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::StopMusic`. Writes whether the handle matched a source.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_stop_music(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    song_handle: i32,
    fade_out_time: f32,
    out: *mut u8,
) -> i32 {
    guard(
        "hp2_rs_audio_stop_music",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host), Some(out)) =
                (sub_mut(sub), host_copy(host), out.as_mut())
            else {
                return 0;
            };
            match sub.stop_music(&mut host, song_handle, fade_out_time) {
                Ok(stopped) => {
                    *out = stopped as u8;
                    1
                }
                Err(HostFailed) => 0,
            }
        }),
    )
}

/// `UALAudioSubsystem::StopAllMusic`.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_stop_all_music(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
    fade_out_time: f32,
) -> i32 {
    guard(
        "hp2_rs_audio_stop_all_music",
        0,
        AssertUnwindSafe(move || unsafe {
            let (Some(sub), Some(mut host)) = (sub_mut(sub), host_copy(host)) else {
                return 0;
            };
            status_of(sub.stop_all_music(&mut host, fade_out_time))
        }),
    )
}

/// `UALAudioSubsystem::PauseSounds`. Cannot fail: only OpenAL calls.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_pause_sounds(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
) {
    guard(
        "hp2_rs_audio_pause_sounds",
        (),
        AssertUnwindSafe(move || unsafe {
            if let (Some(sub), Some(mut host)) = (sub_mut(sub), host_copy(host)) {
                sub.pause_sounds(&mut host);
            }
        }),
    )
}

/// `UALAudioSubsystem::UnpauseSounds`. Cannot fail: only OpenAL calls.
///
/// # Safety
/// See `hp2_rs_audio_init`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_unpause_sounds(
    sub: *mut FfiAudioSubsystem,
    host: *const FfiHost,
) {
    guard(
        "hp2_rs_audio_unpause_sounds",
        (),
        AssertUnwindSafe(move || unsafe {
            if let (Some(sub), Some(mut host)) = (sub_mut(sub), host_copy(host)) {
                sub.unpause_sounds(&mut host);
            }
        }),
    )
}

/// One entry of `UALAudioSubsystem::GetFidelityActiveSources`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiFidelitySource {
    /// The `USound*` whose path name the shell reports.
    pub sound: usize,
    /// 1 when the voice is paused.
    pub paused: u8,
}

/// `UALAudioSubsystem::GetFidelityActiveSources`. Writes up to `cap` entries
/// and returns the total number of active sources. Pass `cap = 0` to count.
///
/// # Safety
/// `out` must be valid for `cap` `FfiFidelitySource` writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_fidelity_sources(
    sub: *const FfiAudioSubsystem,
    out: *mut FfiFidelitySource,
    cap: usize,
) -> usize {
    guard(
        "hp2_rs_audio_fidelity_sources",
        0,
        AssertUnwindSafe(move || unsafe {
            let Some(sub) = sub.cast::<Subsystem>().as_ref() else {
                return 0;
            };
            let active = sub.fidelity_active_sources();
            if !out.is_null() {
                for (slot, (sound, paused)) in active.iter().take(cap).enumerate() {
                    out.add(slot).write(FfiFidelitySource {
                        sound: *sound,
                        paused: *paused as u8,
                    });
                }
            }
            active.len()
        }),
    )
}

/// `UALAudioSubsystem::GetViewport`. Returns the viewport handle, or 0.
///
/// # Safety
/// `sub` must be null or a live pointer from `hp2_rs_audio_subsystem_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_viewport(sub: *const FfiAudioSubsystem) -> usize {
    guard(
        "hp2_rs_audio_viewport",
        0,
        AssertUnwindSafe(move || unsafe {
            sub.cast::<Subsystem>()
                .as_ref()
                .map_or(0, |sub| sub.viewport)
        }),
    )
}

/// Reads `Subsystem::initialized`, so the shell can mirror the original
/// `if (Initialized)` guards in `Destroy` and `ShutdownAfterError`.
///
/// # Safety
/// `sub` must be null or a live pointer from `hp2_rs_audio_subsystem_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_audio_is_initialized(sub: *const FfiAudioSubsystem) -> u8 {
    guard(
        "hp2_rs_audio_is_initialized",
        0,
        AssertUnwindSafe(move || unsafe {
            u8::from(
                sub.cast::<Subsystem>()
                    .as_ref()
                    .is_some_and(|sub| sub.initialized),
            )
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec3_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiVec3>(), 12);
        assert_eq!(std::mem::offset_of!(FfiVec3, z), 8);
    }

    #[test]
    fn audio_config_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiAudioConfig>(), 28);
        assert_eq!(std::mem::offset_of!(FfiAudioConfig, max_channels), 16);
        assert_eq!(
            std::mem::offset_of!(FfiAudioConfig, max_radius_multiplier),
            20
        );
        assert_eq!(std::mem::offset_of!(FfiAudioConfig, use_precache), 24);
        assert_eq!(std::mem::offset_of!(FfiAudioConfig, reverse_stereo), 27);
    }

    #[test]
    fn sound_info_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiSoundInfo>(), 28);
        assert_eq!(std::mem::offset_of!(FfiSoundInfo, raw_sample_rate), 20);
        assert_eq!(std::mem::offset_of!(FfiSoundInfo, is_ogg), 24);
    }

    #[test]
    fn wave_info_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiWaveInfo>(), 32);
        assert_eq!(std::mem::offset_of!(FfiWaveInfo, sample_data), 16);
        assert_eq!(std::mem::offset_of!(FfiWaveInfo, sample_data_size), 24);
    }

    #[test]
    fn vorbis_info_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiVorbisInfo>(), 24);
        assert_eq!(std::mem::offset_of!(FfiVorbisInfo, rate), 8);
        assert_eq!(std::mem::offset_of!(FfiVorbisInfo, time_total), 16);
    }

    #[test]
    fn actor_info_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiActorInfo>(), 72);
        assert_eq!(std::mem::offset_of!(FfiActorInfo, velocity), 12);
        assert_eq!(std::mem::offset_of!(FfiActorInfo, ambient_sound), 32);
        assert_eq!(std::mem::offset_of!(FfiActorInfo, actor_level), 56);
        assert_eq!(std::mem::offset_of!(FfiActorInfo, sound_pitch), 64);
        assert_eq!(std::mem::offset_of!(FfiActorInfo, light_brightness), 68);
    }

    #[test]
    fn viewport_info_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiViewportInfo>(), 56);
        assert_eq!(std::mem::offset_of!(FfiViewportInfo, view_target), 8);
        assert_eq!(
            std::mem::offset_of!(FfiViewportInfo, viewport_actor_level),
            32
        );
        assert_eq!(
            std::mem::offset_of!(FfiViewportInfo, view_target_location),
            40
        );
    }

    #[test]
    fn pop_completed_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiPopCompleted>(), 24);
        assert_eq!(std::mem::offset_of!(FfiPopCompleted, dest), 8);
        assert_eq!(std::mem::offset_of!(FfiPopCompleted, terminal), 16);
    }

    #[test]
    fn play_request_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiPlayRequest>(), 64);
        assert_eq!(std::mem::offset_of!(FfiPlayRequest, sound), 16);
        assert_eq!(std::mem::offset_of!(FfiPlayRequest, location), 24);
        assert_eq!(std::mem::offset_of!(FfiPlayRequest, flags), 48);
        assert_eq!(std::mem::offset_of!(FfiPlayRequest, in_priority), 56);
    }

    #[test]
    fn update_view_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiUpdateView>(), 48);
        assert_eq!(std::mem::offset_of!(FfiUpdateView, z_axis), 36);
    }

    #[test]
    fn fidelity_source_mirror_layout() {
        assert_eq!(std::mem::size_of::<FfiFidelitySource>(), 16);
        assert_eq!(std::mem::offset_of!(FfiFidelitySource, sound), 0);
        assert_eq!(std::mem::offset_of!(FfiFidelitySource, paused), 8);
    }

    #[test]
    fn exec_command_is_u32_like_the_cpp_enum() {
        assert_eq!(std::mem::size_of::<FfiExecCommand>(), 4);
        assert_eq!(FfiExecCommand::PauseSounds as u32, 0);
        assert_eq!(FfiExecCommand::UnpauseSounds as u32, 1);
        assert_eq!(FfiExecCommand::RestartSounds as u32, 2);
        assert_eq!(FfiExecCommand::PlayingSoundStats as u32, 3);
    }

    /// `ctx` plus 74 function pointers, one per `Host` trait method. No
    /// padding, no extra fields.
    #[test]
    fn host_table_has_ctx_plus_seventy_four_entries() {
        assert_eq!(
            std::mem::size_of::<FfiHost>(),
            75 * std::mem::size_of::<usize>()
        );
        assert_eq!(std::mem::offset_of!(FfiHost, ctx), 0);
        assert_eq!(std::mem::offset_of!(FfiHost, log), 8);
        assert_eq!(std::mem::offset_of!(FfiHost, alc_close_device), 74 * 8);
    }

    #[test]
    fn exec_codes_map_to_exec_commands() {
        assert_eq!(
            exec_command(FfiExecCommand::PauseSounds as u32),
            Some(ExecCommand::PauseSounds)
        );
        assert_eq!(
            exec_command(FfiExecCommand::UnpauseSounds as u32),
            Some(ExecCommand::UnpauseSounds)
        );
        assert_eq!(
            exec_command(FfiExecCommand::RestartSounds as u32),
            Some(ExecCommand::RestartSounds)
        );
        assert_eq!(
            exec_command(FfiExecCommand::PlayingSoundStats as u32),
            Some(ExecCommand::PlayingSoundStats)
        );
        assert_eq!(exec_command(4), None);
    }

    #[test]
    fn log_kind_codes_match_shell_categories() {
        assert_eq!(log_kind_code(LogKind::Init), 0);
        assert_eq!(log_kind_code(LogKind::Exit), 1);
        assert_eq!(log_kind_code(LogKind::DevSound), 2);
        assert_eq!(log_kind_code(LogKind::DevMusic), 3);
        assert_eq!(log_kind_code(LogKind::Warning), 4);
        assert_eq!(log_kind_code(LogKind::Log), 5);
    }

    #[test]
    fn config_conversion_maps_every_field() {
        let raw = FfiAudioConfig {
            doppler_factor: 1.5,
            music_volume: 0.25,
            sound_volume: 0.75,
            roll_off: 2.0,
            max_channels: 32,
            max_radius_multiplier: 100.0,
            use_precache: 1,
            use_eax: 0,
            use_mmsystem: 1,
            reverse_stereo: 1,
        };
        let cfg = Config::from(&raw);
        assert_eq!(cfg.doppler_factor, 1.5);
        assert_eq!(cfg.music_volume, 0.25);
        assert_eq!(cfg.sound_volume, 0.75);
        assert_eq!(cfg.roll_off, 2.0);
        assert_eq!(cfg.max_channels, 32);
        assert_eq!(cfg.max_radius_multiplier, 100.0);
        assert!(cfg.use_precache);
        assert!(!cfg.use_eax);
        assert!(cfg.use_mmsystem);
        assert!(cfg.reverse_stereo);
    }
}
