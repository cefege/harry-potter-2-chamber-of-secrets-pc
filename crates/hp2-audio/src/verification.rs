//! Deterministic verification mode for parity/oracle runs.
//!
//! The mode is a process-start choice: `HP2_DETERMINISTIC_VERIFICATION=1`
//! read once into a [`LazyLock`], so no frame ever re-reads the environment.
//! Unset (or any value other than `1`) leaves every path in this module inert
//! and production audio follows the existing wall-clock/default-device path
//! unchanged.
//!
//! When enabled, the subsystem's normal `alcOpenDevice`/`alcCreateContext`
//! calls are routed to a real `ALC_SOFT_loopback` device and context instead
//! of a sound-card device. [`advance`] then steps that device through
//! `alcRenderSamplesSOFT` once per game tick, so queued OpenAL buffers are
//! mixed and consumed by the actual OpenAL Soft mixer (real decoded PCM
//! buffers and real AL source states), not by an emulated playing-state cache.
//! [`seconds`] exposes the tick-driven clock that the audio
//! `Host::app_seconds` callbacks use in this mode; it advances with
//! [`advance`] even when no sound is playing.
//!
//! A run is only "verified" when a loopback context really became current:
//! exactly one machine-readable `<HP2_VERIFY> audio=loopback ...` line is
//! written to stderr at that point. Initialization failures are retained and
//! returned from [`advance`], and no marker is emitted, so a run that fell
//! back to a real device (or to no audio) cannot look verified.

use std::sync::{LazyLock, Mutex, MutexGuard};

#[cfg(feature = "openal")]
use std::sync::atomic::{AtomicBool, Ordering};

/// Process-start environment variable that selects deterministic verification.
pub const ENV: &str = "HP2_DETERMINISTIC_VERIFICATION";

/// Output channel count of the loopback render format (OpenAL mixes every
/// source into this stereo pair).
#[cfg(feature = "openal")]
const OUTPUT_CHANNELS: i32 = 2;
/// Render format used for the loopback device, matching the production
/// subsystem's signed 16-bit PCM path.
#[cfg(feature = "openal")]
const SAMPLE_RATE_DEFAULT: i32 = 44_100;
/// Frames rendered per `alcRenderSamplesSOFT` call. The scratch buffer is a
/// fixed stack array, so verification ticks never allocate PCM storage.
#[cfg(feature = "openal")]
const RENDER_CHUNK_FRAMES: usize = 1024;

#[cfg(feature = "openal")]
const ALC_FORMAT_CHANNELS_SOFT: i32 = 0x1990;
#[cfg(feature = "openal")]
const ALC_FORMAT_TYPE_SOFT: i32 = 0x1991;
#[cfg(feature = "openal")]
const ALC_STEREO_SOFT: i32 = 0x1501;
#[cfg(feature = "openal")]
const ALC_SHORT_SOFT: i32 = 0x1402;

#[cfg_attr(not(feature = "openal"), allow(dead_code))]
struct State {
    /// Deterministic tick clock in seconds.
    seconds: f64,
    /// Sub-frame remainder carried between ticks so `advance` renders exactly
    /// the tick's worth of samples at the loopback rate.
    frame_fraction: f64,
    /// Live `ALCdevice` from `alcLoopbackOpenDeviceSOFT`, as a pointer value.
    device: usize,
    /// Context created on that device, kept until destroy.
    context: usize,
    /// True while that context is the current context.
    current: bool,
    sample_rate: i32,
    /// Sticky initialization/rendering failure, surfaced by [`advance`].
    failure: Option<String>,
}

static STATE: Mutex<State> = Mutex::new(State {
    seconds: 0.0,
    frame_fraction: 0.0,
    device: 0,
    context: 0,
    current: false,
    sample_rate: 0,
    failure: None,
});

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

/// Cached process-start mode choice; the environment is read exactly once.
static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var_os(ENV).is_some_and(|value| value.to_str() == Some("1")));

/// Whether this process was started in deterministic verification mode.
pub fn enabled() -> bool {
    *ENABLED
}

/// Deterministic test clock, or `None` outside deterministic verification mode.
///
/// Advances by exactly the deltas passed to [`advance`], independent of sound
/// activity and of the host wall clock.
pub fn seconds() -> Option<f64> {
    if !enabled() {
        return None;
    }
    Some(state().seconds)
}

/// The same tick clock at the actual visual timestamp boundary. Activation is
/// announced once so the harness can reject older input/audio-only binaries.
pub fn visual_seconds() -> Option<f64> {
    let seconds = seconds()?;
    static ANNOUNCED: std::sync::Once = std::sync::Once::new();
    ANNOUNCED.call_once(|| eprintln!("<HP2_VERIFY> visuals=tick"));
    Some(seconds)
}

/// Steps verification audio by one real game tick.
///
/// With the mode off this is a no-op. With it on, `delta_seconds` advances the
/// deterministic clock, accumulates integral sample frames (fractional
/// remainders carry to the next tick), and renders them through the real
/// loopback device. A missing/failed loopback context is an explicit error,
/// never a silent wall-clock or disabled-audio fallback. Call it on the thread
/// that owns the current OpenAL context (the game/audio thread), once per
/// tick, before the tick's audio update.
pub fn advance(delta_seconds: f32) -> Result<(), String> {
    if !enabled() {
        return Ok(());
    }
    advance_enabled(delta_seconds)
}

#[cfg(not(feature = "openal"))]
fn advance_enabled(_delta_seconds: f32) -> Result<(), String> {
    Err("deterministic verification audio requires the OpenAL backend \
         (hp2-audio `openal` feature), but this build has no loopback support"
        .to_owned())
}

#[cfg(feature = "openal")]
fn advance_enabled(delta_seconds: f32) -> Result<(), String> {
    use crate::openal::ffi;
    use std::ffi::c_void;

    if !delta_seconds.is_finite() || delta_seconds < 0.0 {
        return Err(format!(
            "deterministic verification tick delta must be finite and nonnegative, got {delta_seconds}"
        ));
    }

    let mut state = state();
    if let Some(failure) = state.failure.as_deref() {
        return Err(failure.to_owned());
    }
    if state.device == 0 || !state.current || state.sample_rate <= 0 {
        return Err(
            "deterministic verification audio has no current ALC_SOFT_loopback context".to_owned(),
        );
    }

    state.seconds += f64::from(delta_seconds);
    state.frame_fraction += f64::from(delta_seconds) * f64::from(state.sample_rate);
    let frames = state.frame_fraction.floor();
    state.frame_fraction -= frames;
    let mut remaining = frames as u64;
    if remaining == 0 {
        return Ok(());
    }

    let device = state.device as *mut ffi::AlcDevice;
    let mut scratch = [0i16; RENDER_CHUNK_FRAMES * OUTPUT_CHANNELS as usize];
    while remaining > 0 {
        let count = remaining.min(RENDER_CHUNK_FRAMES as u64) as i32;
        unsafe {
            ffi::alcRenderSamplesSOFT(
                device,
                scratch.as_mut_ptr().cast::<c_void>(),
                count,
            );
        }
        remaining -= count as u64;
    }
    let error = unsafe { ffi::alcGetError(device) };
    if error != 0 {
        let failure =
            format!("ALC_SOFT_loopback rendering failed (ALC error {error})");
        state.failure = Some(failure.clone());
        return Err(failure);
    }
    Ok(())
}

/// Routes a verification-mode `alcOpenDevice` to a real loopback device.
/// Returns 0 (and records why) when loopback support is unavailable.
#[cfg(feature = "openal")]
pub(crate) fn open_device() -> usize {
    use crate::openal::ffi;

    let mut state = state();
    if state.device != 0 {
        return state.device;
    }
    state.failure = None;
    let present = unsafe {
        ffi::alcIsExtensionPresent(std::ptr::null_mut(), c"ALC_SOFT_loopback".as_ptr())
    };
    if present == 0 {
        state.failure = Some(
            "deterministic verification requires ALC_SOFT_loopback, which this OpenAL build \
             does not expose"
                .to_owned(),
        );
        return 0;
    }
    let device = unsafe { ffi::alcLoopbackOpenDeviceSOFT(std::ptr::null()) } as usize;
    if device == 0 {
        state.failure =
            Some("ALC_SOFT_loopback device creation failed".to_owned());
        return 0;
    }
    state.device = device;
    state.context = 0;
    state.current = false;
    state.sample_rate = 0;
    device
}

/// Creates the verification context on the loopback device. The requested
/// frequency is honored; loopback format attributes replace the caller's
/// device attributes (loopback rejects `ALC_REFRESH`/`ALC_SYNC`).
#[cfg(feature = "openal")]
pub(crate) fn create_context(device: usize, attributes: &[i32]) -> usize {
    use crate::openal::ffi;

    let mut rate = SAMPLE_RATE_DEFAULT;
    let mut index = 0;
    while index + 1 < attributes.len() {
        let key = attributes[index];
        if key == 0 {
            break;
        }
        if key == crate::al::ALC_FREQUENCY {
            rate = attributes[index + 1];
        }
        index += 2;
    }
    if rate <= 0 {
        state().failure = Some(format!("invalid loopback sample rate {rate}"));
        return 0;
    }

    let mut state = state();
    if device == 0 || state.device != device {
        state.failure =
            Some("deterministic verification context requested for an unknown device".to_owned());
        return 0;
    }
    let supported = unsafe {
        ffi::alcIsRenderFormatSupportedSOFT(
            device as *mut ffi::AlcDevice,
            rate,
            ALC_STEREO_SOFT,
            ALC_SHORT_SOFT,
        )
    };
    if supported == 0 {
        state.failure = Some(format!(
            "ALC_SOFT_loopback does not support {rate} Hz {OUTPUT_CHANNELS}-channel signed \
             16-bit PCM"
        ));
        return 0;
    }
    let loopback_attributes = [
        ALC_FORMAT_CHANNELS_SOFT,
        ALC_STEREO_SOFT,
        ALC_FORMAT_TYPE_SOFT,
        ALC_SHORT_SOFT,
        crate::al::ALC_FREQUENCY,
        rate,
        0,
    ];
    let context = unsafe {
        ffi::alcCreateContext(device as *mut ffi::AlcDevice, loopback_attributes.as_ptr())
    } as usize;
    if context == 0 {
        let error = unsafe { ffi::alcGetError(device as *mut ffi::AlcDevice) };
        state.failure = Some(format!(
            "ALC_SOFT_loopback context creation failed (ALC error {error})"
        ));
        return 0;
    }
    state.context = context;
    state.sample_rate = rate;
    state.failure = None;
    context
}

/// Makes the verification context current and, on the first real success,
/// emits the one-line activation marker the harness requires.
#[cfg(feature = "openal")]
pub(crate) fn make_context_current(context: usize) {
    use crate::openal::ffi;

    let mut state = state();
    if context == 0 {
        state.current = false;
        unsafe { ffi::alcMakeContextCurrent(std::ptr::null_mut()) };
        return;
    }
    if state.context != context {
        state.failure =
            Some("deterministic verification make-current on an unknown context".to_owned());
        return;
    }
    let made_current =
        unsafe { ffi::alcMakeContextCurrent(context as *mut ffi::AlcContext) } != 0;
    if !made_current {
        let error = unsafe { ffi::alcGetError(state.device as *mut ffi::AlcDevice) };
        state.current = false;
        state.failure = Some(format!(
            "ALC_SOFT_loopback make-current failed (ALC error {error})"
        ));
        return;
    }
    state.current = true;
    let rate = state.sample_rate;
    static ANNOUNCED: AtomicBool = AtomicBool::new(false);
    if !ANNOUNCED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "<HP2_VERIFY> audio=loopback rate={rate} channels={OUTPUT_CHANNELS} format=short"
        );
    }
}

/// Destroys the verification context, leaving the loopback device open until
/// [`close_device`] (the same ordering the subsystem uses for real devices).
#[cfg(feature = "openal")]
pub(crate) fn destroy_context(context: usize) {
    use crate::openal::ffi;

    let mut state = state();
    if context != 0 {
        unsafe { ffi::alcDestroyContext(context as *mut ffi::AlcContext) };
    }
    if state.context == context {
        state.context = 0;
        state.current = false;
        state.sample_rate = 0;
    }
}

/// Closes the loopback device and drops all verification state for it.
#[cfg(feature = "openal")]
pub(crate) fn close_device(device: usize) {
    use crate::openal::ffi;

    let mut state = state();
    if device != 0 {
        unsafe { ffi::alcCloseDevice(device as *mut ffi::AlcDevice) };
    }
    if state.device == device {
        state.device = 0;
        state.context = 0;
        state.current = false;
        state.sample_rate = 0;
    }
}

#[cfg(all(test, feature = "openal"))]
mod tests {
    use super::*;
    use crate::{al, openal};

    #[test]
    fn fractional_ticks_consume_real_pcm_and_teardown_rejects_advance() {
        assert!(enabled(), "this integration contract requires verification mode");
        let device = openal::alc_open_device();
        assert_ne!(device, 0);
        // The live device and zero-terminated attributes meet the ALC contract.
        let context = unsafe {
            openal::alc_create_context(device, &[al::ALC_FREQUENCY, 44_100, 0])
        };
        assert_ne!(context, 0);
        openal::alc_make_context_current(context);

        let source = openal::al_gen_source();
        let mut buffers = [0];
        openal::al_gen_buffers(&mut buffers);
        let pcm: Vec<u8> = (0..100i16).flat_map(i16::to_ne_bytes).collect();
        openal::al_buffer_data(buffers[0], al::AL_FORMAT_MONO16, &pcm, 44_100);
        openal::al_source_queue_buffers(source, &buffers);
        openal::al_source_play(source);
        assert_eq!(openal::al_get_error(), 0);

        // Each tick is less than one sample: dropping fractional frames would
        // leave this real 100-frame source playing forever.
        let delta = 0.25f32 / 44_100.0;
        let start = seconds().unwrap();
        for _ in 0..396 {
            advance(delta).unwrap();
        }
        assert_eq!(openal::al_get_source_i(source, al::AL_SOURCE_STATE), al::AL_PLAYING);
        for _ in 0..8 {
            advance(delta).unwrap();
        }
        assert_eq!(openal::al_get_source_i(source, al::AL_SOURCE_STATE), al::AL_STOPPED);
        assert!((seconds().unwrap() - start - f64::from(delta) * 404.0).abs() < 1e-12);
        assert!(advance(f32::NAN).is_err());
        assert!(advance(-0.02).is_err());

        openal::al_delete_source(source);
        openal::al_delete_buffers(&buffers);
        openal::alc_make_context_current(0);
        openal::alc_destroy_context(context);
        openal::alc_close_device(device);
        assert!(advance(0.02).is_err());
    }
}
