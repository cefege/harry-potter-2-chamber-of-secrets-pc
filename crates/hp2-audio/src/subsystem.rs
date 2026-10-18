//! ALAudio subsystem state and lifecycle, ported from
//! `HarryPotter2/Unreal/ALAudio/Src/ALAudioSubsystem.cpp` at 06bfdd3:
//! `Init`, `Destroy`, `ShutdownAfterError`, `SetViewport`, `RegisterSound`,
//! `UnregisterSound`, and `GetNewStreamStruct`.
//!
//! Rust owns the `Sources`, `Buffers`, and `Streams` tables that the original
//! kept on `UALAudioSubsystem`, plus every OpenAL call made on them. Engine
//! state (`USound`, `AActor`, `ULevel`, `UViewport`, file streams, the ogg
//! decoder) is reached only through [`Host`]. The C++ shell keeps the UObject,
//! its config properties, and the function table that implements `Host`.

use crate::al::{
    AL_FORMAT_MONO8, AL_FORMAT_MONO16, AL_FORMAT_STEREO8, AL_FORMAT_STEREO16,
    AL_INVERSE_DISTANCE_CLAMPED, al_error,
};
pub use crate::ambient::SF_LOOPING;
use crate::fade::FadeMode;
use crate::host::{Host, HostFailed, HostResult, LogKind, SoundInfo};
use crate::lipsync::LipSync;
use crate::stream::{BufferState, MAX_BUFFERS_PER_STREAM, StreamState};
use crate::vec3::Vec3;

/// `MAX_AUDIOCHANNELS` (Init caps `MaxChannels` at this).
pub const MAX_AUDIOCHANNELS: usize = 32;
/// `MAX_AUDIOSTREAMS` (size of the stream slot table).
pub const MAX_AUDIOSTREAMS: usize = 32;
/// `OGGVORBIS_STREAM_CHUNKSIZE`.
pub const OGGVORBIS_STREAM_CHUNKSIZE: i32 = 65536;
/// `XA_STREAM_CHUNKSIZE`.
pub const XA_STREAM_CHUNKSIZE: i32 = 32768;

/// `SF_Streaming` (`UnAudio.h`).
pub const SF_STREAMING: i32 = 4;
/// `SF_Music`.
pub const SF_MUSIC: i32 = 8;
/// `SF_No3D`.
pub const SF_NO3D: i32 = 16;
/// `SF_NoUpdates`.
pub const SF_NO_UPDATES: i32 = 64;
/// `SF_HasLipSync`.
pub const SF_HAS_LIPSYNC: i32 = 128;

/// `0xDEADBEEF` as the `INT` that `USound::Handle` holds while a sound is
/// being registered (and left behind by a failed ogg/XA registration).
pub const HANDLE_PENDING: i32 = 0xDEAD_BEEFu32 as i32;

/// Config values that the C++ shell passes on every call. The UObject keeps
/// the properties; Rust reads these values and writes `roll_off` back (Init).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub doppler_factor: f32,
    pub music_volume: f32,
    pub sound_volume: f32,
    pub roll_off: f32,
    pub max_channels: i32,
    pub use_precache: bool,
    /// `GAudioMaxRadiusMultiplier`.
    pub max_radius_multiplier: f32,
    /// `UseEAX`. Together with `use_mmsystem` selects the DS3D listener orientation branch.
    pub use_eax: bool,
    /// `CompatibilityMode` (MMSYSTEM).
    pub use_mmsystem: bool,
    /// `ReverseStereo`.
    pub reverse_stereo: bool,
}

/// One OpenAL voice (`ALSource` in the original).
#[derive(Debug)]
pub struct Source {
    /// `USound*` as an opaque handle; 0 is `NULL`.
    pub sound: usize,
    pub source: u32,
    /// `AActor*` as an opaque handle; 0 is `NULL`.
    pub actor: usize,
    pub location: Vec3,
    pub priority: f32,
    pub radius: f32,
    pub zone_radius: f32,
    pub used_radius: f32,
    pub wanted_radius: f32,
    /// `FTime` tick count of the last update.
    pub last_change: i64,
    pub volume: f32,
    pub fade_duration: f32,
    pub fade_time: f32,
    pub fade_mode: FadeMode,
    pub flags: i32,
    pub id: i32,
    pub started: bool,
    pub paused: bool,
    pub lip_sync: Option<LipSync>,
}

impl Source {
    /// A zeroed voice bound to an OpenAL source name (`AddZeroed` + assignment).
    pub fn empty(source: u32) -> Self {
        Source {
            sound: 0,
            source,
            actor: 0,
            location: [0.0; 3],
            priority: 0.0,
            radius: 0.0,
            zone_radius: 0.0,
            used_radius: 0.0,
            wanted_radius: 0.0,
            last_change: 0,
            volume: 0.0,
            fade_duration: 0.0,
            fade_time: 0.0,
            fade_mode: FadeMode::None,
            flags: 0,
            id: 0,
            started: false,
            paused: false,
            lip_sync: None,
        }
    }
}

/// One OpenAL buffer record (`ALBuffer`).
#[derive(Debug, Default)]
pub struct Buffer {
    pub buffer: u32,
    pub flags: i32,
    pub name: String,
}

/// One streaming slot (`ALStream`). `id` is the engine stream id plus one; 0
/// marks a free slot.
#[derive(Debug)]
pub struct Stream {
    pub st: StreamState,
    /// Owns the refill buffers whose addresses `st.refill` hands to the host.
    pub refill_data: [Option<Box<[u8]>>; MAX_BUFFERS_PER_STREAM],
    pub id: i32,
    pub flags: i32,
    pub processed: i32,
    pub rate: i32,
    pub format: i32,
    pub stream_chunk_size: i32,
    pub name: String,
}

impl Stream {
    /// `appMemzero` of an `ALStream`: every field zero, which leaves each
    /// `BufferState` at `QueuedPending` (discriminant 0).
    pub fn empty() -> Self {
        Stream {
            st: StreamState {
                buffers: [0; MAX_BUFFERS_PER_STREAM],
                refill: [0; MAX_BUFFERS_PER_STREAM],
                state: [BufferState::QueuedPending; MAX_BUFFERS_PER_STREAM],
                num_buffers: 0,
                alive: false,
                reset: false,
            },
            refill_data: [None, None, None],
            id: 0,
            flags: 0,
            processed: 0,
            rate: 0,
            format: 0,
            stream_chunk_size: 0,
            name: String::new(),
        }
    }

    /// Stores refill buffer `i`, keeping its address in `st.refill[i]` (0 when absent).
    fn set_refill(&mut self, i: usize, data: Option<Box<[u8]>>) {
        self.st.refill[i] = data.as_ref().map_or(0, |b| b.as_ptr() as usize);
        self.refill_data[i] = data;
    }
}

/// The subsystem state. The C++ shell owns one boxed instance per
/// `UALAudioSubsystem` and calls the methods below through FFI.
#[derive(Debug)]
pub struct Subsystem {
    pub initialized: bool,
    pub device: usize,
    pub context: usize,
    /// `UViewport*` as an opaque handle; 0 is `NULL`.
    pub viewport: usize,
    /// Only `ZoneNumber` of the original `RegionListener` is used.
    pub region_listener_zone: i32,
    pub last_position: Vec3,
    /// `LastTime` as an `FTime` tick count.
    pub last_time: i64,
    pub last_realtime: bool,
    pub last_paused: bool,
    pub restart_sounds: bool,
    /// Static `FreeSlot` of `PlaySound`.
    pub free_slot: i32,
    pub sources: Vec<Source>,
    pub buffers: Vec<Buffer>,
    pub streams: Vec<Stream>,
}

impl Subsystem {
    /// The constructor. `last_time` is `appSeconds()` read by the C++ shell.
    pub fn new(last_time: i64) -> Self {
        Subsystem {
            initialized: false,
            device: 0,
            context: 0,
            viewport: 0,
            region_listener_zone: 0,
            last_position: [0.0; 3],
            last_time,
            last_realtime: false,
            last_paused: false,
            restart_sounds: false,
            free_slot: 0,
            sources: Vec::new(),
            buffers: Vec::new(),
            streams: Vec::new(),
        }
    }

    /// `UALAudioSubsystem::GetFidelityActiveSources`: every started source
    /// that has a sound, as `(sound handle, paused)`, in source order.
    pub fn fidelity_active_sources(&self) -> Vec<(usize, bool)> {
        self.sources
            .iter()
            .filter(|source| source.sound != 0 && source.started)
            .map(|source| (source.sound, source.paused))
            .collect()
    }

    /// `UALAudioSubsystem::Init`. Returns `Ok(false)` on the original's
    /// `return 0` paths. Writes `roll_off` back into `config`.
    pub fn init<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        config: &mut Config,
    ) -> HostResult<bool> {
        if self.initialized {
            return Ok(true);
        }
        self.device = host.alc_open_device();
        if self.device == 0 {
            host.log(LogKind::Init, "ALAudio: no OpenAL devices found.")?;
            return Ok(false);
        }
        let attributes = [crate::al::ALC_FREQUENCY, 44100, 0];
        self.context = host.alc_create_context(self.device, &attributes);
        if self.context == 0 {
            host.log(LogKind::Init, "ALAudio: context creation failed.")?;
            return Ok(false);
        }
        host.alc_make_context_current(self.context);
        if al_error(host, "Init", true)? {
            host.log(LogKind::Init, "ALAudio: makecurrent failed.")?;
            return Ok(false);
        }
        if config.max_channels == 0 {
            host.log(LogKind::Init, "MaxChannels == 0")?;
            return Ok(false);
        }

        self.sources.clear();
        let limit = config.max_channels.min(MAX_AUDIOCHANNELS as i32).max(0);
        for _ in 0..limit {
            let source = host.al_gen_source();
            if al_error(host, "Init (creating sources)", false)? {
                break;
            }
            self.sources.push(Source::empty(source));
        }
        if self.sources.is_empty() {
            host.log(LogKind::Init, "ALAudio: couldn't allocate sources")?;
            return Ok(false);
        }

        self.streams = (0..MAX_AUDIOSTREAMS).map(|_| Stream::empty()).collect();
        if config.roll_off <= 0.0 {
            config.roll_off = 1.0;
        }
        host.al_distance_model(AL_INVERSE_DISTANCE_CLAMPED);
        host.al_doppler_factor(config.doppler_factor);

        self.initialized = true;
        self.last_realtime = false;
        self.last_paused = false;
        host.log(LogKind::Init, "ALAudio: subsystem initialized.")?;
        Ok(true)
    }

    /// `UALAudioSubsystem::Destroy` (the Super::Destroy call and the
    /// `USound::Audio` clearing are done by the C++ shell).
    pub fn destroy<H: Host + ?Sized>(&mut self, host: &mut H, config: &Config) -> HostResult<()> {
        if !self.initialized {
            return Ok(());
        }
        for i in 0..self.sources.len() {
            self.stop_sound_source(host, i)?;
            host.al_delete_source(self.sources[i].source);
        }
        self.sources.clear();

        if self.viewport != 0 {
            let count = host.sound_count()?;
            let mut sounds = vec![0usize; count];
            host.sound_fill(&mut sounds)?;
            for sound in sounds {
                self.unregister_sound(host, sound, -1)?;
            }
        }

        for buffer in &self.buffers {
            if buffer.buffer != 0 {
                host.al_delete_buffers(&[buffer.buffer]);
            }
        }
        self.buffers.clear();

        for stream in &self.streams {
            if stream.id != 0 {
                host.gfile_destroy_stream(stream.id - 1)?;
            }
            if stream.st.num_buffers != 0 {
                host.al_delete_buffers(&stream.st.buffers[..stream.st.num_buffers]);
            }
        }
        // Dropping the streams releases the refill buffers.
        self.streams.clear();

        host.alc_make_context_current(0);
        host.alc_destroy_context(self.context);
        host.alc_close_device(self.device);
        self.set_viewport(host, config, 0)?;
        host.log(LogKind::Exit, "OpenAL Audio subsystem shut down.")?;
        Ok(())
    }

    /// `UALAudioSubsystem::ShutdownAfterError` (the `USound::Audio` clearing
    /// is done by the C++ shell).
    pub fn shutdown_after_error<H: Host + ?Sized>(&mut self, host: &mut H) -> HostResult<()> {
        if self.initialized {
            host.log(LogKind::Exit, "UALAudioSubsystem::ShutdownAfterError")?;
        }
        Ok(())
    }

    /// `UALAudioSubsystem::SetViewport`.
    pub fn set_viewport<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        config: &Config,
        viewport: usize,
    ) -> HostResult<()> {
        for i in 0..self.sources.len() {
            if self.sources[i].started && (self.sources[i].flags & SF_MUSIC) == 0 {
                self.stop_sound_source(host, i)?;
            }
        }
        if self.viewport != viewport {
            self.viewport = viewport;
            self.region_listener_zone = 0;
            if self.viewport != 0 && config.use_precache {
                let count = host.sound_count()?;
                let mut sounds = vec![0usize; count];
                host.sound_fill(&mut sounds)?;
                for sound in sounds {
                    self.register_sound(host, sound)?;
                }
            }
        }
        Ok(())
    }

    /// `UALAudioSubsystem::RegisterSound`. Returns whether the sound is
    /// usable. A failed ogg/XA registration returns `false` without bumping
    /// `RefCount`, as the original does.
    pub fn register_sound<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        sound: usize,
    ) -> HostResult<bool> {
        if !host.sound_is_registered(sound)? {
            // Avoid recursion as USound->Load calls RegisterSound.
            host.sound_set_handle(sound, HANDLE_PENDING)?;
            let info = host.sound_info(sound)?;
            let flags = info.core_flags | info.working_flags;
            if (flags & SF_STREAMING) != 0 {
                let started = if info.is_ogg {
                    self.register_ogg(host, sound, flags)?
                } else {
                    self.register_xa(host, sound, flags, &info)?
                };
                if !started {
                    return Ok(false);
                }
            } else {
                self.register_wave(host, sound)?;
            }
        }
        // Track how many times a sound object is SUCCESSFULLY registered.
        host.sound_add_ref(sound, 1)?;
        Ok(true)
    }

    /// Ogg Vorbis branch of `RegisterSound`.
    fn register_ogg<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        sound: usize,
        flags: i32,
    ) -> HostResult<bool> {
        let chunk = OGGVORBIS_STREAM_CHUNKSIZE;
        let chunk_len = chunk as usize;
        let num = MAX_BUFFERS_PER_STREAM;
        let mut prime = vec![0u8; chunk_len * num];
        let refill: [Option<Box<[u8]>>; MAX_BUFFERS_PER_STREAM] =
            std::array::from_fn(|_| Some(vec![0u8; chunk_len].into_boxed_slice()));

        let decoder = host.vorbis_new()?;
        if decoder == 0 {
            return Ok(false);
        }
        let looping = (flags & SF_MUSIC) != 0;
        let id =
            host.gfile_create_ogg_stream(sound, chunk, num as i32, &mut prime, looping, decoder)?;
        if id < 0 {
            let filename = host.sound_filename(sound)?;
            host.log(
                LogKind::DevSound,
                &format!("Failed to register sound: {filename}"),
            )?;
            host.vorbis_free(decoder)?;
            return Ok(false);
        }
        let info = host.vorbis_info(decoder)?;
        if info.pcm_total < chunk as i64 * num as i64 {
            let filename = host.sound_filename(sound)?;
            host.log(
                LogKind::DevSound,
                &format!("Sound is too short for streaming: {filename}"),
            )?;
        }
        host.sound_set_duration(sound, info.time_total as f32)?;

        let index = self.get_new_stream_struct(host)? + 1;
        if index == 0 {
            let filename = host.sound_filename(sound)?;
            host.log(
                LogKind::DevSound,
                &format!("Failed to register sound: {filename}"),
            )?;
            // A successful CreateStream transferred the decoder to the stream.
            host.gfile_destroy_stream(id)?;
            return Ok(false);
        }
        host.sound_add_stream_handle(sound, index)?;
        let path = host.sound_path_name(sound)?;

        let stream = &mut self.streams[(index - 1) as usize];
        stream.flags = flags;
        stream.id = id + 1;
        stream.processed = 0;
        stream.st.alive = true;
        stream.st.reset = false;
        stream.rate = info.rate;
        stream.name = path;
        stream.stream_chunk_size = chunk;
        stream.st.num_buffers = num;
        for (i, data) in refill.into_iter().enumerate().take(num) {
            stream.set_refill(i, data);
            stream.st.state[i] = BufferState::QueuedNoPrefetch;
        }

        host.al_gen_buffers(&mut stream.st.buffers[..num]);
        al_error(host, "Creating streaming buffer.", true)?;
        let format = match info.channels {
            1 => AL_FORMAT_MONO16,
            2 => AL_FORMAT_STEREO16,
            _ => {
                stream.format = 0;
                host.app_error("Invalid sound data")?;
                return Err(HostFailed);
            }
        };
        stream.format = format;
        for i in 0..num {
            host.al_buffer_data(
                stream.st.buffers[i],
                format,
                &prime[i * chunk_len..(i + 1) * chunk_len],
                stream.rate,
            );
        }
        for i in 0..num {
            if host.gfile_request_chunk(id, stream.st.refill[i])? {
                stream.st.state[i] = BufferState::QueuedPending;
            }
        }
        Ok(true)
    }

    /// XA branch of `RegisterSound`.
    fn register_xa<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        sound: usize,
        flags: i32,
        info: &SoundInfo,
    ) -> HostResult<bool> {
        let decoded_length = info
            .raw_num_samples
            .wrapping_mul(info.raw_bits_per_sample / 8)
            .wrapping_mul(info.raw_num_channels);
        let decoded_chunks = if decoded_length == 0 {
            0
        } else {
            decoded_length.wrapping_sub(1) / XA_STREAM_CHUNKSIZE + 1
        };
        let looping = (flags & SF_LOOPING) != 0;
        let max = MAX_BUFFERS_PER_STREAM as i32;
        let num_buffers = if looping {
            max
        } else {
            decoded_chunks.min(max)
        };
        let needs_ongoing = looping || decoded_chunks > max;

        let chunk = XA_STREAM_CHUNKSIZE;
        let chunk_len = chunk as usize;
        let num = num_buffers.max(0) as usize;
        let mut prime = vec![0u8; chunk_len * num];
        let refill: [Option<Box<[u8]>>; MAX_BUFFERS_PER_STREAM] = std::array::from_fn(|i| {
            if needs_ongoing && i < num {
                Some(vec![0u8; chunk_len].into_boxed_slice())
            } else {
                None
            }
        });

        let id = host.gfile_create_xa_stream(
            sound,
            info.raw_num_samples,
            chunk,
            num as i32,
            &mut prime,
            looping,
        )?;
        if id < 0 {
            let name = host.sound_name(sound)?;
            host.log(
                LogKind::DevSound,
                &format!("Failed to register sound: {name}"),
            )?;
            return Ok(false);
        }
        let index = self.get_new_stream_struct(host)? + 1;
        if index == 0 {
            let name = host.sound_name(sound)?;
            host.log(
                LogKind::DevSound,
                &format!("Failed to register sound: {name}"),
            )?;
            host.gfile_destroy_stream(id)?;
            return Ok(false);
        }
        host.sound_add_stream_handle(sound, index)?;
        let path = host.sound_path_name(sound)?;

        let stream = &mut self.streams[(index - 1) as usize];
        stream.flags = flags;
        stream.id = id + 1;
        stream.processed = 0;
        stream.st.alive = needs_ongoing;
        stream.st.reset = false;
        stream.rate = info.raw_sample_rate;
        stream.name = path;
        stream.stream_chunk_size = chunk;
        stream.st.num_buffers = num;
        for (i, data) in refill.into_iter().enumerate().take(num) {
            stream.set_refill(i, data);
            stream.st.state[i] = BufferState::QueuedNoPrefetch;
        }

        host.al_gen_buffers(&mut stream.st.buffers[..num]);
        al_error(host, "Creating streaming buffer.", true)?;
        let format = if info.raw_bits_per_sample == 8 {
            let warning = format!("WARNING: 8 bit sound detected [{}]", stream.name);
            host.log(LogKind::DevSound, &warning)?;
            if info.raw_num_channels == 1 {
                AL_FORMAT_MONO8
            } else {
                AL_FORMAT_STEREO8
            }
        } else if info.raw_num_channels == 1 {
            AL_FORMAT_MONO16
        } else {
            AL_FORMAT_STEREO16
        };
        stream.format = format;
        for i in 0..num {
            host.al_buffer_data(
                stream.st.buffers[i],
                format,
                &prime[i * chunk_len..(i + 1) * chunk_len],
                stream.rate,
            );
        }
        for i in 0..num {
            if stream.st.refill[i] != 0 && host.gfile_request_chunk(id, stream.st.refill[i])? {
                stream.st.state[i] = BufferState::QueuedPending;
            }
        }
        Ok(true)
    }

    /// Wave branch of `RegisterSound`: loads the samples, makes one static
    /// buffer, and unloads the samples. Only the handle is set on failure.
    fn register_wave<H: Host + ?Sized>(&mut self, host: &mut H, sound: usize) -> HostResult<()> {
        host.sound_load_data(sound)?;
        let wave = host.sound_wave_info(sound)?;
        let flags = if wave.sample_loops_num != 0 {
            SF_LOOPING
        } else {
            0
        };
        let format = if wave.bits_per_sample == 8 {
            let path = host.sound_path_name(sound)?;
            host.log(
                LogKind::DevSound,
                &format!("WARNING: 8 bit sound detected [{path}]"),
            )?;
            if wave.channels == 1 {
                AL_FORMAT_MONO8
            } else {
                AL_FORMAT_STEREO8
            }
        } else if wave.channels == 1 {
            AL_FORMAT_MONO16
        } else {
            AL_FORMAT_STEREO16
        };

        let mut bid = [0u32; 1];
        host.al_gen_buffers(&mut bid);
        al_error(host, "RegisterSound (generating buffer)", true)?;
        host.al_buffer_data_at(
            bid[0],
            format,
            wave.sample_data,
            wave.sample_data_size,
            wave.samples_per_sec,
        );
        host.sound_unload_data(sound)?;

        if al_error(host, "RegisterSound (creating buffer)", true)? {
            host.sound_set_handle(sound, 0)?;
        } else {
            let name = host.sound_path_name(sound)?;
            self.buffers.push(Buffer {
                buffer: bid[0],
                flags,
                name,
            });
            host.sound_set_handle(sound, self.buffers.len() as i32)?;
        }
        Ok(())
    }

    /// `UALAudioSubsystem::UnregisterSound`. `id == -1` matches every voice
    /// playing `sound`; otherwise voices with that id.
    pub fn unregister_sound<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        sound: usize,
        id: i32,
    ) -> HostResult<()> {
        for i in 0..self.sources.len() {
            let matches = if id == -1 {
                self.sources[i].sound != 0 && self.sources[i].sound == sound
            } else {
                self.sources[i].sound != 0 && self.sources[i].id == id
            };
            if matches {
                self.old_stop_sound(host, i)?;
            }
        }

        al_error(host, "", false)?;
        let count = host.sound_add_ref(sound, -1)?;
        if host.sound_get_handle(sound, id)? != 0 {
            let working_flags = host.sound_info(sound)?.working_flags;
            if (working_flags & SF_STREAMING) != 0 {
                if id == -1 {
                    let n = host.sound_playing_stream_count(sound)?;
                    let mut handles = vec![0i32; n];
                    host.sound_playing_stream_fill(sound, &mut handles)?;
                    for handle in handles {
                        self.destroy_stream_slot(host, (handle - 1) as usize)?;
                    }
                    host.sound_unbind_all_streams(sound)?;
                    host.sound_delete_all_stream_handles(sound)?;
                } else {
                    let handle = host.sound_get_handle(sound, id)?;
                    self.destroy_stream_slot(host, (handle - 1) as usize)?;
                    host.sound_unbind_stream(sound, id)?;
                    host.sound_delete_stream_handle(sound, handle)?;
                }
            } else if count == 0 {
                let i = (host.sound_get_handle(sound, -1)? - 1) as usize;
                let buffer = self.buffers[i].buffer;
                host.al_delete_buffers(&[buffer]);
                self.buffers[i].buffer = 0;
                self.buffers[i].flags = 0;
                host.sound_set_handle(sound, 0)?;
            }
        }
        Ok(())
    }

    /// Stops and frees one stream slot: destroys its engine stream, deletes its
    /// OpenAL buffers, then memzeroes the record (which frees the refill data).
    fn destroy_stream_slot<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        slot: usize,
    ) -> HostResult<()> {
        let stream = &mut self.streams[slot];
        host.gfile_destroy_stream(stream.id - 1)?;
        let n = stream.st.num_buffers;
        host.al_delete_buffers(&stream.st.buffers[..n]);
        *stream = Stream::empty();
        Ok(())
    }

    /// `UALAudioSubsystem::GetNewStreamStruct`: claims the first free stream
    /// slot and returns its index, or -1 when all are in use.
    pub fn get_new_stream_struct<H: Host + ?Sized>(&mut self, host: &mut H) -> HostResult<i32> {
        for (i, stream) in self.streams.iter_mut().enumerate() {
            if stream.id == 0 {
                *stream = Stream::empty();
                return Ok(i as i32);
            }
        }
        host.log(
            LogKind::DevSound,
            &format!("More than {MAX_AUDIOSTREAMS} streams in use"),
        )?;
        Ok(-1)
    }
}
