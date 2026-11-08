//! Canonical Core implementation of the audio Host boundary.

use hp2_audio::host::{Host, HostFailed, HostResult, LogKind};
use hp2_core::core_access::{CoreAccess, RuntimeError};
#[path = "audio_core_host_sound.rs"]
mod sound;
#[path = "audio_core_host_stream.rs"]
mod stream;
#[path = "audio_core_host_world.rs"]
mod world;
use crate::actor_anim_owner as lipsync;

/// Services audio operations using canonical Core records and Rust SDK resources.
pub(crate) struct CoreHost<'a> {
    pub(super) access: CoreAccess<'a>,
    pub(super) error: Option<RuntimeError>,
    output: Option<&'a mut crate::exec_output::ExecOutput<'a>>,
}

impl<'a> CoreHost<'a> {
    /// Creates a host for a synchronous operation without an Exec output device.
    pub(crate) fn new(access: CoreAccess<'a>) -> Self {
        Self { access, error: None, output: None }
    }

    /// Creates a host whose command output is routed to the caller's Ar sink.
    pub(crate) fn with_output(
        access: CoreAccess<'a>,
        output: &'a mut crate::exec_output::ExecOutput<'a>,
    ) -> Self {
        Self { access, error: None, output: Some(output) }
    }

    /// Converts the first host failure into the owned Core runtime diagnostic.
    pub(crate) fn take_error(&mut self, context: &str) -> RuntimeError {
        self.error.take().unwrap_or_else(|| RuntimeError::new(context)).with_context(context)
    }

    pub(crate) fn audio_radius_multiplier(&self) -> f32 {
        // SAFETY: shared process global; this engine-thread read mirrors native access.
        unsafe { hp2_core::audio_globals::GAudioMaxRadiusMultiplier }
    }

    pub(super) fn fail<T>(&mut self, context: &str) -> HostResult<T> {
        if self.error.is_none() {
            self.error = Some(RuntimeError::new(context));
        }
        Err(HostFailed)
    }

    fn log_units(text: &str) -> Vec<u32> {
        text.chars().map(u32::from).chain(std::iter::once(0)).collect()
    }
}

impl Host for CoreHost<'_> {
    fn log(&mut self, kind: LogKind, text: &str) -> HostResult<()> {
        let category = match kind {
            LogKind::Init => "Init",
            LogKind::Exit => "Exit",
            LogKind::DevSound => "DevSound",
            LogKind::DevMusic => "DevMusic",
            LogKind::Warning => "Warning",
            LogKind::Log => "",
        };
        let name = if category.is_empty() {
            0
        } else {
            match self.access.intern_str(category, hp2_core::name_owner::FindName::Add) {
                Ok(name) => name,
                Err(error) => return Err(self.fail_with(error)),
            }
        };
        let message = Self::log_units(text);
        self.access.emit_log(name, &message).map_err(|error| {
            if self.error.is_none() { self.error = Some(error); }
            HostFailed
        })
    }

    fn output(&mut self, text: &str) -> HostResult<()> {
        let Some(output) = self.output.as_deref_mut() else {
            return self.fail("Audio command output is unavailable");
        };
        let message = Self::log_units(text);
        output(&message).map_err(|error| {
            if self.error.is_none() { self.error = Some(error); }
            HostFailed
        })
    }

    fn app_seconds(&mut self) -> HostResult<i64> {
        let seconds = hp2_audio::verification::seconds()
            .unwrap_or_else(hp2_core::unix_clock::seconds_slow);
        Ok(hp2_core::time::from_double(seconds))
    }

    fn app_error(&mut self, text: &str) -> HostResult<()> {
        let message = Self::log_units(text);
        let category = match self.access.intern_str("Error", hp2_core::name_owner::FindName::Add) {
            Ok(category) => category,
            Err(error) => return Err(self.fail_with(error)),
        };
        if let Err(error) = self.access.emit_log(category, &message) {
            return Err(self.fail_with(error));
        }
        Err(self.fail_with(RuntimeError::new(text).with_context("Audio fatal error")))
    }
    fn al_get_error(&mut self) -> i32 { hp2_audio::openal::al_get_error() }
    fn al_gen_source(&mut self) -> u32 { hp2_audio::openal::al_gen_source() }
    fn al_delete_source(&mut self, source: u32) { hp2_audio::openal::al_delete_source(source) }
    fn al_gen_buffers(&mut self, out: &mut [u32]) { hp2_audio::openal::al_gen_buffers(out) }
    fn al_delete_buffers(&mut self, buffers: &[u32]) { hp2_audio::openal::al_delete_buffers(buffers) }
    fn al_buffer_data(&mut self, buffer: u32, format: i32, data: &[u8], frequency: i32) {
        hp2_audio::openal::al_buffer_data(buffer, format, data, frequency)
    }
    fn al_buffer_data_at(&mut self, buffer: u32, format: i32, data: usize, size: i32, frequency: i32) {
        // SAFETY: `data` is the live canonical sample pointer supplied by the sound record.
        unsafe { hp2_audio::openal::al_buffer_data_at(buffer, format, data, size, frequency) }
    }
    fn al_buffer_size(&mut self, buffer: u32) -> i32 { hp2_audio::openal::al_buffer_size(buffer) }
    fn al_source_queue_buffers(&mut self, source: u32, buffers: &[u32]) {
        hp2_audio::openal::al_source_queue_buffers(source, buffers)
    }
    fn al_source_unqueue_buffer(&mut self, source: u32) -> u32 {
        hp2_audio::openal::al_source_unqueue_buffer(source)
    }
    fn al_get_source_i(&mut self, source: u32, param: i32) -> i32 {
        hp2_audio::openal::al_get_source_i(source, param)
    }
    fn al_source_i(&mut self, source: u32, param: i32, value: i32) {
        hp2_audio::openal::al_source_i(source, param, value)
    }
    fn al_source_f(&mut self, source: u32, param: i32, value: f32) {
        hp2_audio::openal::al_source_f(source, param, value)
    }
    fn al_source_fv(&mut self, source: u32, param: i32, value: hp2_audio::vec3::Vec3) {
        hp2_audio::openal::al_source_fv(source, param, value)
    }
    fn al_source_play(&mut self, source: u32) { hp2_audio::openal::al_source_play(source) }
    fn al_source_pause(&mut self, source: u32) { hp2_audio::openal::al_source_pause(source) }
    fn al_source_stop(&mut self, source: u32) { hp2_audio::openal::al_source_stop(source) }
    fn al_source_rewind(&mut self, source: u32) { hp2_audio::openal::al_source_rewind(source) }
    fn al_listener_fv(&mut self, param: i32, value: &[f32]) {
        // SAFETY: callers pass vectors with the exact OpenAL field width.
        unsafe { hp2_audio::openal::al_listener_fv(param, value) }
    }
    fn al_distance_model(&mut self, model: i32) { hp2_audio::openal::al_distance_model(model) }
    fn al_doppler_factor(&mut self, factor: f32) { hp2_audio::openal::al_doppler_factor(factor) }
    fn alc_open_device(&mut self) -> usize { hp2_audio::openal::alc_open_device() }
    fn alc_create_context(&mut self, device: usize, attributes: &[i32]) -> usize {
        // SAFETY: audio subsystem supplies a NUL-terminated attribute list.
        unsafe { hp2_audio::openal::alc_create_context(device, attributes) }
    }
    fn alc_make_context_current(&mut self, context: usize) {
        hp2_audio::openal::alc_make_context_current(context)
    }
    fn alc_destroy_context(&mut self, context: usize) { hp2_audio::openal::alc_destroy_context(context) }
    fn alc_close_device(&mut self, device: usize) { hp2_audio::openal::alc_close_device(device) }
    fn sound_info(&mut self, sound: usize) -> HostResult<hp2_audio::host::SoundInfo> { self.host_sound_info(sound) }
    fn sound_data_len(&mut self, sound: usize) -> HostResult<i32> { self.host_sound_data_len(sound) }
    fn sound_name(&mut self, sound: usize) -> HostResult<String> { self.host_sound_name(sound) }
    fn sound_path_name(&mut self, sound: usize) -> HostResult<String> { self.host_sound_path_name(sound) }
    fn sound_filename(&mut self, sound: usize) -> HostResult<String> { self.host_sound_filename(sound) }
    fn sound_set_working_flags(&mut self, sound: usize, flags: i32) -> HostResult<()> { self.host_sound_set_working_flags(sound, flags) }
    fn sound_set_handle(&mut self, sound: usize, handle: i32) -> HostResult<()> { self.host_sound_set_handle(sound, handle) }
    fn sound_get_handle(&mut self, sound: usize, id: i32) -> HostResult<i32> { self.host_sound_get_handle(sound, id) }
    fn sound_set_duration(&mut self, sound: usize, duration: f32) -> HostResult<()> { self.host_sound_set_duration(sound, duration) }
    fn sound_add_ref(&mut self, sound: usize, delta: i32) -> HostResult<i32> { self.host_sound_add_ref(sound, delta) }
    fn sound_is_registered(&mut self, sound: usize) -> HostResult<bool> { self.host_sound_is_registered(sound) }
    fn sound_add_stream_handle(&mut self, sound: usize, id: i32) -> HostResult<()> { self.host_sound_add_stream_handle(sound, id) }
    fn sound_delete_stream_handle(&mut self, sound: usize, handle: i32) -> HostResult<()> { self.host_sound_delete_stream_handle(sound, handle) }
    fn sound_bind_stream(&mut self, sound: usize, id: i32) -> HostResult<()> { self.host_sound_bind_stream(sound, id) }
    fn sound_unbind_stream(&mut self, sound: usize, id: i32) -> HostResult<()> { self.host_sound_unbind_stream(sound, id) }
    fn sound_unbind_all_streams(&mut self, sound: usize) -> HostResult<()> { self.host_sound_unbind_all_streams(sound) }
    fn sound_delete_all_stream_handles(&mut self, sound: usize) -> HostResult<()> { self.host_sound_delete_all_stream_handles(sound) }
    fn sound_playing_stream_count(&mut self, sound: usize) -> HostResult<usize> { self.host_sound_playing_stream_count(sound) }
    fn sound_playing_stream_fill(&mut self, sound: usize, out: &mut [i32]) -> HostResult<()> { self.host_sound_playing_stream_fill(sound, out) }
    fn sound_load_data(&mut self, sound: usize) -> HostResult<()> { self.host_sound_load_data(sound) }
    fn sound_unload_data(&mut self, sound: usize) -> HostResult<()> { self.host_sound_unload_data(sound) }
    fn sound_wave_info(&mut self, sound: usize) -> HostResult<hp2_audio::host::WaveInfo> { self.host_sound_wave_info(sound) }
    fn sound_count(&mut self) -> HostResult<usize> { self.host_sound_count() }
    fn sound_fill(&mut self, out: &mut [usize]) -> HostResult<()> { self.host_sound_fill(out) }
    fn music_sound_new(&mut self, filename: &str) -> HostResult<usize> { self.host_music_sound_new(filename) }
    fn vorbis_new(&mut self) -> HostResult<usize> { self.host_vorbis_new() }
    fn vorbis_free(&mut self, decoder: usize) -> HostResult<()> { self.host_vorbis_free(decoder) }
    fn vorbis_info(&mut self, decoder: usize) -> HostResult<hp2_audio::host::VorbisInfo> { self.host_vorbis_info(decoder) }
    fn gfile_create_ogg_stream(&mut self, sound: usize, chunk: i32, num: i32, prime: &mut [u8], looping: bool, decoder: usize) -> HostResult<i32> {
        self.host_gfile_create_ogg_stream(sound, chunk, num, prime, looping, decoder)
    }
    fn gfile_create_xa_stream(&mut self, sound: usize, num_samples: i32, chunk: i32, num: i32, prime: &mut [u8], looping: bool) -> HostResult<i32> {
        self.host_gfile_create_xa_stream(sound, num_samples, chunk, num, prime, looping)
    }
    fn gfile_destroy_stream(&mut self, id: i32) -> HostResult<()> { self.host_gfile_destroy_stream(id) }
    fn gfile_request_chunk(&mut self, id: i32, dest: usize) -> HostResult<bool> { self.host_gfile_request_chunk(id, dest) }
    fn gfile_pop_completed_chunk(&mut self, id: i32) -> HostResult<Option<(usize, bool)>> {
        self.host_gfile_pop_completed_chunk(id)
    }
    fn actor_info(&mut self, actor: usize) -> HostResult<hp2_audio::host::ActorInfo> {
        self.world_actor_info(actor)
    }
    fn actor_is_owned_by(&mut self, actor: usize, owner: usize) -> HostResult<bool> {
        self.world_actor_is_owned_by(actor, owner)
    }
    fn level_is_audible_at(
        &mut self,
        level: usize,
        sound_location: hp2_audio::vec3::Vec3,
        listener_location: hp2_audio::vec3::Vec3,
        actor: usize,
        occlusion: u8,
    ) -> HostResult<bool> {
        self.world_level_is_audible_at(level, sound_location, listener_location, actor, occlusion)
    }
    fn level_point_region_zone(&mut self, level: usize, actor_level: usize, location: hp2_audio::vec3::Vec3) -> HostResult<i32> {
        self.world_level_point_region_zone(level, actor_level, location)
    }
    fn level_radius_multiplier(&mut self, level: usize, zone_a: i32, zone_b: i32) -> HostResult<f32> {
        self.world_level_radius_multiplier(level, zone_a, zone_b)
    }
    fn viewport_info(&mut self, viewport: usize) -> HostResult<hp2_audio::host::ViewportInfo> {
        self.world_viewport_info(viewport)
    }
    fn level_actor_count(&mut self, level: usize) -> HostResult<usize> {
        self.world_level_actor_count(level)
    }
    fn level_actor_fill(&mut self, level: usize, out: &mut [usize]) -> HostResult<()> {
        self.world_level_actor_fill(level, out)
    }
    fn lipsync_set_frame(&mut self, channel: usize, sound: usize, elapsed: f32) -> HostResult<()> {
        self.world_lipsync_set_frame(channel, sound, elapsed)
    }
    fn lipsync_start(&mut self, actor: usize) -> HostResult<usize> {
        match lipsync::lipsync_start(self.access, actor as *mut hp2_abi::engine::generated::AActor) {
            Ok(channel) => Ok(channel as usize),
            Err(error) => Err(self.fail_with(error)),
        }
    }
    fn lipsync_release(&mut self, channel: usize) -> HostResult<()> {
        match lipsync::lipsync_release(self.access, channel as *mut hp2_abi::engine::generated::AActor) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.fail_with(error)),
        }
    }
}

impl CoreHost<'_> {
    pub(super) fn fail_with(&mut self, error: RuntimeError) -> HostFailed {
        if self.error.is_none() { self.error = Some(error); }
        HostFailed
    }
}

