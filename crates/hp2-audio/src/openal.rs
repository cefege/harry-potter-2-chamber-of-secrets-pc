//! Direct OpenAL/ALC FFI calls, replacing the `AlHostAl*`/`AlHostAlc*`
//! trampolines in `ALAudioSubsystem.cpp` (`:691-855`) and their `MakeHost`
//! assignments (`:915-940`). Each function below wraps exactly one
//! `al*`/`alc*` call and matches the `Host` trait's own `al_*`/`alc_*`
//! method signatures (`crates/hp2-audio/src/host.rs:231-266`) so
//! `impl Host for FfiHost` (`crates/hp2-ffi/src/audio.rs`) can forward to
//! these one-for-one with no translation.
//!
//! **Feature-gated like `hp2-platform`'s `sdl` feature** (see that crate's
//! `src/clipboard.rs` doc comment for the full design reasoning this
//! mirrors): OpenAL is a CMake-`FetchContent`-built shared dylib
//! (`Build/CMake/HP2Dependencies.cmake`'s `openal-soft`, `LIBTYPE SHARED`),
//! not present on a bare `cargo build`/`cargo test` outside the CMake tree.
//! The `openal` Cargo feature is off by default so this crate builds and
//! tests standalone with no OpenAL binary to find; only CMake's build
//! (`Build/CMake/HP2Targets.cmake`'s `HP2_FFI_FEATURES`) turns it on.
//!
//! Every function below exists unconditionally with the same name and
//! signature either way, mirroring `hp2-platform::client_policy`'s
//! `sdl_quit` dual-stub: with the feature on, the body makes the real
//! `al*`/`alc*` call; without it, a no-op/zero-returning stub. Callers
//! (`impl Host for FfiHost`) never need their own `#[cfg]`.
//!
//! `AL_SIZE` (used by [`al_buffer_size`]'s `alGetBufferi` call) is
//! [`crate::al::AL_SIZE`], not redeclared here.
//!
//! With the `openal` feature on and deterministic verification mode enabled
//! (`HP2_DETERMINISTIC_VERIFICATION=1`, see [`crate::verification`]), the five
//! device/context entry points below route to a real `ALC_SOFT_loopback`
//! device instead of the default output device. Every `al*` call is unchanged
//! and operates on that loopback context. With the mode off, these functions
//! call the same OpenAL entry points as before.

#[cfg(feature = "openal")]
pub(crate) mod ffi {
    use std::ffi::{c_char, c_int, c_void};

    /// Opaque `ALCdevice`; every caller only ever holds a pointer to this.
    #[repr(C)]
    pub struct AlcDevice {
        _private: [u8; 0],
    }

    /// Opaque `ALCcontext`; every caller only ever holds a pointer to this.
    #[repr(C)]
    pub struct AlcContext {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn alGetError() -> c_int;
        pub fn alGenSources(n: c_int, sources: *mut u32);
        pub fn alDeleteSources(n: c_int, sources: *const u32);
        pub fn alGenBuffers(n: c_int, buffers: *mut u32);
        pub fn alDeleteBuffers(n: c_int, buffers: *const u32);
        pub fn alBufferData(
            buffer: u32,
            format: c_int,
            data: *const c_void,
            size: c_int,
            freq: c_int,
        );
        pub fn alGetBufferi(buffer: u32, param: c_int, value: *mut c_int);
        pub fn alSourceQueueBuffers(source: u32, n: c_int, buffers: *const u32);
        pub fn alSourceUnqueueBuffers(source: u32, n: c_int, buffers: *mut u32);
        pub fn alGetSourcei(source: u32, param: c_int, value: *mut c_int);
        pub fn alSourcei(source: u32, param: c_int, value: c_int);
        pub fn alSourcef(source: u32, param: c_int, value: f32);
        pub fn alSourcefv(source: u32, param: c_int, values: *const f32);
        pub fn alSourcePlay(source: u32);
        pub fn alSourcePause(source: u32);
        pub fn alSourceStop(source: u32);
        pub fn alSourceRewind(source: u32);
        pub fn alListenerfv(param: c_int, values: *const f32);
        pub fn alDistanceModel(model: c_int);
        pub fn alDopplerFactor(value: f32);
        pub fn alcOpenDevice(devicename: *const c_char) -> *mut AlcDevice;
        pub fn alcCreateContext(device: *mut AlcDevice, attrlist: *const c_int) -> *mut AlcContext;
        pub fn alcMakeContextCurrent(context: *mut AlcContext) -> u8;
        pub fn alcDestroyContext(context: *mut AlcContext);
        pub fn alcCloseDevice(device: *mut AlcDevice) -> u8;
        pub fn alcGetError(device: *mut AlcDevice) -> c_int;
        pub fn alcIsExtensionPresent(device: *mut AlcDevice, extname: *const c_char) -> u8;
        pub fn alcLoopbackOpenDeviceSOFT(devicename: *const c_char) -> *mut AlcDevice;
        pub fn alcIsRenderFormatSupportedSOFT(
            device: *mut AlcDevice,
            frequency: c_int,
            channels: c_int,
            kind: c_int,
        ) -> u8;
        pub fn alcRenderSamplesSOFT(device: *mut AlcDevice, buffer: *mut c_void, samples: c_int);
    }
}

/// Replaces `AlHostAlGetError` (`ALAudioSubsystem.cpp:691-695`): `alGetError()`.
#[cfg(feature = "openal")]
pub fn al_get_error() -> i32 {
    unsafe { ffi::alGetError() as i32 }
}
#[cfg(not(feature = "openal"))]
pub fn al_get_error() -> i32 {
    0
}

/// Replaces `AlHostAlGenSource` (`ALAudioSubsystem.cpp:697-703`):
/// `alGenSources(1, &S)`.
#[cfg(feature = "openal")]
pub fn al_gen_source() -> u32 {
    let mut source = 0u32;
    unsafe { ffi::alGenSources(1, &mut source) };
    source
}
#[cfg(not(feature = "openal"))]
pub fn al_gen_source() -> u32 {
    0
}

/// Replaces `AlHostAlDeleteSource` (`ALAudioSubsystem.cpp:705-710`):
/// `alDeleteSources(1, &S)`.
#[cfg(feature = "openal")]
pub fn al_delete_source(source: u32) {
    unsafe { ffi::alDeleteSources(1, &source) };
}
#[cfg(not(feature = "openal"))]
pub fn al_delete_source(_source: u32) {}

/// Replaces `AlHostAlGenBuffers` (`ALAudioSubsystem.cpp:712-716`):
/// `alGenBuffers(n, out)`.
#[cfg(feature = "openal")]
pub fn al_gen_buffers(out: &mut [u32]) {
    unsafe { ffi::alGenBuffers(out.len() as i32, out.as_mut_ptr()) };
}
#[cfg(not(feature = "openal"))]
pub fn al_gen_buffers(_out: &mut [u32]) {}

/// Replaces `AlHostAlDeleteBuffers` (`ALAudioSubsystem.cpp:718-722`):
/// `alDeleteBuffers(n, buffers)`.
#[cfg(feature = "openal")]
pub fn al_delete_buffers(buffers: &[u32]) {
    unsafe { ffi::alDeleteBuffers(buffers.len() as i32, buffers.as_ptr()) };
}
#[cfg(not(feature = "openal"))]
pub fn al_delete_buffers(_buffers: &[u32]) {}

/// Replaces `AlHostAlBufferData` (`ALAudioSubsystem.cpp:724-728`):
/// `alBufferData(buffer, format, data, len, frequency)`.
#[cfg(feature = "openal")]
pub fn al_buffer_data(buffer: u32, format: i32, data: &[u8], frequency: i32) {
    unsafe {
        ffi::alBufferData(
            buffer,
            format,
            data.as_ptr().cast(),
            data.len() as i32,
            frequency,
        )
    };
}
#[cfg(not(feature = "openal"))]
pub fn al_buffer_data(_buffer: u32, _format: i32, _data: &[u8], _frequency: i32) {}

/// Replaces `AlHostAlBufferDataAt` (`ALAudioSubsystem.cpp:730-734`):
/// `alBufferData(buffer, format, (const ALvoid*)data, size, frequency)`,
/// reading `size` bytes directly from engine memory at `data` (the wave
/// `SampleDataStart`, valid until `sound_unload_data`) instead of a Rust
/// slice.
///
/// # Safety
/// `data` must be a valid pointer (as a `usize`) to at least `size` live
/// bytes of readable memory for the duration of this call.
#[cfg(feature = "openal")]
pub unsafe fn al_buffer_data_at(buffer: u32, format: i32, data: usize, size: i32, frequency: i32) {
    unsafe { ffi::alBufferData(buffer, format, data as *const _, size, frequency) };
}
/// # Safety
/// No preconditions without the `openal` feature: this stub never reads
/// `data`.
#[cfg(not(feature = "openal"))]
pub unsafe fn al_buffer_data_at(
    _buffer: u32,
    _format: i32,
    _data: usize,
    _size: i32,
    _frequency: i32,
) {
}

/// Replaces `AlHostAlBufferSize` (`ALAudioSubsystem.cpp:736-742`):
/// `alGetBufferi(buffer, AL_SIZE, &V)`.
#[cfg(feature = "openal")]
pub fn al_buffer_size(buffer: u32) -> i32 {
    let mut value = 0i32;
    unsafe { ffi::alGetBufferi(buffer, crate::al::AL_SIZE, &mut value) };
    value
}
#[cfg(not(feature = "openal"))]
pub fn al_buffer_size(_buffer: u32) -> i32 {
    0
}

/// Replaces `AlHostAlSourceQueueBuffers` (`ALAudioSubsystem.cpp:744-748`):
/// `alSourceQueueBuffers(source, n, buffers)`.
#[cfg(feature = "openal")]
pub fn al_source_queue_buffers(source: u32, buffers: &[u32]) {
    unsafe { ffi::alSourceQueueBuffers(source, buffers.len() as i32, buffers.as_ptr()) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_queue_buffers(_source: u32, _buffers: &[u32]) {}

/// Replaces `AlHostAlSourceUnqueueBuffer` (`ALAudioSubsystem.cpp:750-756`):
/// `alSourceUnqueueBuffers(source, 1, &B)`.
#[cfg(feature = "openal")]
pub fn al_source_unqueue_buffer(source: u32) -> u32 {
    let mut buffer = 0u32;
    unsafe { ffi::alSourceUnqueueBuffers(source, 1, &mut buffer) };
    buffer
}
#[cfg(not(feature = "openal"))]
pub fn al_source_unqueue_buffer(_source: u32) -> u32 {
    0
}

/// Replaces `AlHostAlGetSourceI` (`ALAudioSubsystem.cpp:758-764`):
/// `alGetSourcei(source, param, &V)`.
#[cfg(feature = "openal")]
pub fn al_get_source_i(source: u32, param: i32) -> i32 {
    let mut value = 0i32;
    unsafe { ffi::alGetSourcei(source, param, &mut value) };
    value
}
#[cfg(not(feature = "openal"))]
pub fn al_get_source_i(_source: u32, _param: i32) -> i32 {
    0
}

/// Replaces `AlHostAlSourceI` (`ALAudioSubsystem.cpp:766-770`):
/// `alSourcei(source, param, value)`.
#[cfg(feature = "openal")]
pub fn al_source_i(source: u32, param: i32, value: i32) {
    unsafe { ffi::alSourcei(source, param, value) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_i(_source: u32, _param: i32, _value: i32) {}

/// Replaces `AlHostAlSourceF` (`ALAudioSubsystem.cpp:772-776`):
/// `alSourcef(source, param, value)`.
#[cfg(feature = "openal")]
pub fn al_source_f(source: u32, param: i32, value: f32) {
    unsafe { ffi::alSourcef(source, param, value) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_f(_source: u32, _param: i32, _value: f32) {}

/// Replaces `AlHostAlSourceFv` (`ALAudioSubsystem.cpp:778-783`):
/// `alSourcefv(source, param, V)` with `V = {value[0], value[1], value[2]}`.
#[cfg(feature = "openal")]
pub fn al_source_fv(source: u32, param: i32, value: crate::vec3::Vec3) {
    unsafe { ffi::alSourcefv(source, param, value.as_ptr()) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_fv(_source: u32, _param: i32, _value: crate::vec3::Vec3) {}

/// Replaces `AlHostAlSourcePlay` (`ALAudioSubsystem.cpp:785-789`):
/// `alSourcePlay(source)`.
#[cfg(feature = "openal")]
pub fn al_source_play(source: u32) {
    unsafe { ffi::alSourcePlay(source) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_play(_source: u32) {}

/// Replaces `AlHostAlSourcePause` (`ALAudioSubsystem.cpp:791-795`):
/// `alSourcePause(source)`.
#[cfg(feature = "openal")]
pub fn al_source_pause(source: u32) {
    unsafe { ffi::alSourcePause(source) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_pause(_source: u32) {}

/// Replaces `AlHostAlSourceStop` (`ALAudioSubsystem.cpp:797-801`):
/// `alSourceStop(source)`.
#[cfg(feature = "openal")]
pub fn al_source_stop(source: u32) {
    unsafe { ffi::alSourceStop(source) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_stop(_source: u32) {}

/// Replaces `AlHostAlSourceRewind` (`ALAudioSubsystem.cpp:803-807`):
/// `alSourceRewind(source)`.
#[cfg(feature = "openal")]
pub fn al_source_rewind(source: u32) {
    unsafe { ffi::alSourceRewind(source) };
}
#[cfg(not(feature = "openal"))]
pub fn al_source_rewind(_source: u32) {}

/// Replaces `AlHostAlListenerFv` (`ALAudioSubsystem.cpp:809-813`):
/// `alListenerfv(param, value)`.
///
/// # Safety
/// `alListenerfv` reads a fixed number of floats from `value` based on
/// `param` (3 for most listener params, 6 for `AL_ORIENTATION`) with no
/// length of its own to bound the read, exactly like the raw pointer the
/// deleted C++ trampoline forwarded. `value` must hold at least as many
/// floats as `param` requires.
#[cfg(feature = "openal")]
pub unsafe fn al_listener_fv(param: i32, value: &[f32]) {
    unsafe { ffi::alListenerfv(param, value.as_ptr()) };
}
/// # Safety
/// No preconditions without the `openal` feature: this stub never reads
/// `value`.
#[cfg(not(feature = "openal"))]
pub unsafe fn al_listener_fv(_param: i32, _value: &[f32]) {}

/// Replaces `AlHostAlDistanceModel` (`ALAudioSubsystem.cpp:815-819`):
/// `alDistanceModel(model)`.
#[cfg(feature = "openal")]
pub fn al_distance_model(model: i32) {
    unsafe { ffi::alDistanceModel(model) };
}
#[cfg(not(feature = "openal"))]
pub fn al_distance_model(_model: i32) {}

/// Replaces `AlHostAlDopplerFactor` (`ALAudioSubsystem.cpp:821-825`):
/// `alDopplerFactor(factor)`.
#[cfg(feature = "openal")]
pub fn al_doppler_factor(factor: f32) {
    unsafe { ffi::alDopplerFactor(factor) };
}
#[cfg(not(feature = "openal"))]
pub fn al_doppler_factor(_factor: f32) {}

/// Replaces `AlHostAlcOpenDevice` (`ALAudioSubsystem.cpp:827-831`):
/// `alcOpenDevice(NULL)`. Returns the device handle as a `usize`, or 0 on
/// failure.
///
/// In deterministic verification mode this opens a real
/// `ALC_SOFT_loopback` device instead (see [`crate::verification`]); the
/// production default-device path is untouched when the mode is off.
#[cfg(feature = "openal")]
pub fn alc_open_device() -> usize {
    if crate::verification::enabled() {
        return crate::verification::open_device();
    }
    unsafe { ffi::alcOpenDevice(std::ptr::null()) as usize }
}
#[cfg(not(feature = "openal"))]
pub fn alc_open_device() -> usize {
    0
}

/// Replaces `AlHostAlcCreateContext` (`ALAudioSubsystem.cpp:833-837`):
/// `alcCreateContext(device, attributes)`.
///
/// # Safety
/// `alcCreateContext` reads `attributes` until a `0` terminator, with no
/// length of its own to bound the read, exactly like the raw pointer the
/// deleted C++ trampoline forwarded. `attributes` must end with a `0`
/// (the `Host` trait's call site in `impl Host for FfiHost` already
/// `debug_assert!`s this).
#[cfg(feature = "openal")]
pub unsafe fn alc_create_context(device: usize, attributes: &[i32]) -> usize {
    if crate::verification::enabled() {
        return crate::verification::create_context(device, attributes);
    }
    unsafe { ffi::alcCreateContext(device as *mut ffi::AlcDevice, attributes.as_ptr()) as usize }
}
/// # Safety
/// No preconditions without the `openal` feature: this stub never reads
/// `attributes`.
#[cfg(not(feature = "openal"))]
pub unsafe fn alc_create_context(_device: usize, _attributes: &[i32]) -> usize {
    0
}

/// Replaces `AlHostAlcMakeContextCurrent` (`ALAudioSubsystem.cpp:839-843`):
/// `alcMakeContextCurrent(context)`, discarding the `ALCboolean` result (the
/// real C++ does too).
#[cfg(feature = "openal")]
pub fn alc_make_context_current(context: usize) {
    if crate::verification::enabled() {
        crate::verification::make_context_current(context);
        return;
    }
    unsafe { ffi::alcMakeContextCurrent(context as *mut ffi::AlcContext) };
}
#[cfg(not(feature = "openal"))]
pub fn alc_make_context_current(_context: usize) {}

/// Replaces `AlHostAlcDestroyContext` (`ALAudioSubsystem.cpp:845-849`):
/// `alcDestroyContext(context)`.
#[cfg(feature = "openal")]
pub fn alc_destroy_context(context: usize) {
    if crate::verification::enabled() {
        crate::verification::destroy_context(context);
        return;
    }
    unsafe { ffi::alcDestroyContext(context as *mut ffi::AlcContext) };
}
#[cfg(not(feature = "openal"))]
pub fn alc_destroy_context(_context: usize) {}

/// Replaces `AlHostAlcCloseDevice` (`ALAudioSubsystem.cpp:851-855`):
/// `alcCloseDevice(device)`, discarding the `ALCboolean` result (the real
/// C++ does too).
#[cfg(feature = "openal")]
pub fn alc_close_device(device: usize) {
    if crate::verification::enabled() {
        crate::verification::close_device(device);
        return;
    }
    unsafe { ffi::alcCloseDevice(device as *mut ffi::AlcDevice) };
}
#[cfg(not(feature = "openal"))]
pub fn alc_close_device(_device: usize) {}
