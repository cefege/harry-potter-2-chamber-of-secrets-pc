//! Canonical Rust ownership for UAudioSubsystem/ALAudioSubsystem and OpenAL resources.

use hp2_abi::engine::generated::{self, UObject, UEngine, UViewport, UALAudioSubsystem};
use hp2_audio::console::{Hp2AudioConsoleLeaf, Hp2AudioConsoleServices};
use hp2_audio::exec::ExecCommand;
use hp2_audio::subsystem::{Config, Subsystem};
use hp2_core::{
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind},
    object_archive::ObjectArchive,
    object_owner::ClassOps,
};
use hp2_audio::host::HostResult;
use std::{alloc::Layout, cell::{Cell, RefCell}, collections::HashMap, ptr, rc::Rc};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_CONFIG: u32 = 0x0000_0004;
const CLASS_ABSTRACT: u32 = 0x0000_0001;
const BASE: ClassOps = NativeRecordKind::Subsystem.operations();

const ALAUDIO_PROPERTIES: &[StartupProperty] = &[
    StartupProperty { name: "UseEAX", kind: StartupPropertyKind::Bool { bit_mask: 1 }, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, use_eax), array_dim: 1 },
    StartupProperty { name: "CompatibilityMode", kind: StartupPropertyKind::Bool { bit_mask: 1 }, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, use_mmsystem), array_dim: 1 },
    StartupProperty { name: "UsePrecache", kind: StartupPropertyKind::Bool { bit_mask: 1 }, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, use_precache), array_dim: 1 },
    StartupProperty { name: "ReverseStereo", kind: StartupPropertyKind::Bool { bit_mask: 1 }, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, reverse_stereo), array_dim: 1 },
    StartupProperty { name: "Channels", kind: StartupPropertyKind::Int, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, max_channels), array_dim: 1 },
    StartupProperty { name: "MusicVolume", kind: StartupPropertyKind::Float, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, music_volume), array_dim: 1 },
    StartupProperty { name: "SoundVolume", kind: StartupPropertyKind::Float, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, sound_volume), array_dim: 1 },
    StartupProperty { name: "DopplerFactor", kind: StartupPropertyKind::Float, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, doppler_factor), array_dim: 1 },
    StartupProperty { name: "RollOff", kind: StartupPropertyKind::Float, category: "ALAudio", property_flags: 0x0000_4000, object_flags: 4, inner_name: None, offset: std::mem::offset_of!(UALAudioSubsystem, roll_off), array_dim: 1 },
];

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "AudioSubsystem", package: "Engine", super_name: Some("Subsystem"),
        within: "Object", flags: CLASS_ABSTRACT | CLASS_CONFIG, object_flags: OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(native_layout::<generated::UAudioSubsystem>(generated::UAudioSubsystem::NATIVE_ALIGNMENT)), operations: Some(BASE),
        properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "ALAudioSubsystem", package: "ALAudio", super_name: Some("Engine.AudioSubsystem"),
        within: "Object", flags: CLASS_CONFIG, object_flags: OBJECT_FLAGS,
        config: "System", guid: [0; 4], kind: NativeRecordKind::Object,
        layout: Some(native_layout::<UALAudioSubsystem>(UALAudioSubsystem::NATIVE_ALIGNMENT)), operations: Some(AL_AUDIO_OPERATIONS),
        properties: ALAUDIO_PROPERTIES, defaults: &[],
    },
];

const fn native_layout<T>(alignment: usize) -> Layout {
    match Layout::from_size_align(std::mem::size_of::<T>(), alignment) {
        Ok(layout) => layout,
        Err(_) => panic!("compiler-probed audio alignment is invalid"),
    }
}

const AL_AUDIO_OPERATIONS: ClassOps = {
    let mut operations = BASE;
    operations.construct = construct_audio;
    operations.destroy = destroy_audio;
    operations.serialize = serialize_audio;
    operations.visit_references = visit_audio;
    operations.post_edit_change = post_edit_change;
    operations
};

unsafe fn construct_audio(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { (BASE.construct)(access, object)?; }
    let audio = object.cast::<UALAudioSubsystem>();
    unsafe {
        ptr::addr_of_mut!((*audio).viewport).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*audio).handle).write_unaligned(ptr::null_mut());
    }
    let now = hp2_core::time::from_double(hp2_core::unix_clock::seconds_slow());
    let owner = match state(access) {
        Ok(owner) => owner,
        Err(error) => {
            unsafe { (BASE.destroy)(access, object)?; }
            return Err(error);
        }
    };
    if let Err(error) = owner.register(audio as usize, now) {
        unsafe { (BASE.destroy)(access, object)?; }
        return Err(error);
    }
    Ok(())
}

unsafe fn destroy_audio(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    let audio = object.cast::<UALAudioSubsystem>();
    shutdown(access, audio)?;
    state(access)?.unregister(audio as usize)?;
    unsafe { (BASE.destroy)(access, object) }
}

unsafe fn serialize_audio(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        (BASE.serialize)(access, object, archive)?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*object.cast::<UALAudioSubsystem>()).viewport).cast())?;
    }
    Ok(())
}

unsafe fn visit_audio(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        (BASE.visit_references)(access, object, archive)?;
        archive.serialize_object_slot(ptr::addr_of_mut!((*object.cast::<UALAudioSubsystem>()).viewport).cast())?;
    }
    Ok(())
}

unsafe fn post_edit_change(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if object.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    let audio = object.cast::<UALAudioSubsystem>();
    unsafe {
        for slot in [
            ptr::addr_of_mut!((*audio).sound_volume),
            ptr::addr_of_mut!((*audio).music_volume),
        ] {
            let value = slot.read_unaligned();
            slot.write_unaligned(if value < 0.0 { 0.0 } else if value > 1.0 { 1.0 } else { value });
        }
        (BASE.post_edit_change)(access, object)
    }
}

/// Owns one audio runtime per canonical audio subsystem object.
#[derive(Default)]
pub(crate) struct AudioOwnerState {
    resources: RefCell<HashMap<usize, Rc<AudioRuntime>>>,
    stream_manager_owned: Cell<bool>,
    stream_users: Cell<usize>,
}

struct AudioRuntime {
    busy: Cell<bool>,
    subsystem: RefCell<Option<Subsystem>>,
}

/// Returns the audio state owned by the current canonical Engine graph.
pub(crate) fn state(access: CoreAccess<'_>) -> RuntimeResult<Rc<AudioOwnerState>> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine audio owner state is unavailable"))?;
    Ok(engine.audio_resources.clone())
}

/// Mirrors the native USound::Audio publication and its lifecycle callbacks.
fn set_sound_audio(access: CoreAccess<'_>, audio: *mut UALAudioSubsystem) -> RuntimeResult<()> {
    let engine = access.owner_context::<crate::core_owner::EngineState>()
        .ok_or_else(|| RuntimeError::new("Canonical Engine audio owner state is unavailable"))?;
    engine.sound_audio.set(std::ptr::NonNull::new(audio.cast::<UObject>()).map(|receiver| {
        crate::core_owner::AudioBinding { receiver, unregister_sound, note_destroy }
    }));
    Ok(())
}

/// Resolves the Engine.Audio subsystem field to the registered canonical
/// ALAudioSubsystem resource without retaining an EngineState borrow.
pub(crate) fn engine_audio(
    access: CoreAccess<'_>,
    engine: std::ptr::NonNull<UEngine>,
) -> RuntimeResult<std::ptr::NonNull<UALAudioSubsystem>> {
    let raw = unsafe { ptr::addr_of!((*engine.as_ptr()).audio).read_unaligned() }
        .cast::<UALAudioSubsystem>();
    let audio = std::ptr::NonNull::new(raw)
        .ok_or_else(|| RuntimeError::new("Engine.Audio has no canonical ALAudioSubsystem"))?;
    state(access)?.with_subsystem(audio.as_ptr() as usize, |_| Ok(()))?;
    Ok(audio)
}

/// Counts the actual started sound sources in the native fidelity source order.
pub fn fidelity_active_source_count(
    access: CoreAccess<'_>,
    audio: std::ptr::NonNull<UALAudioSubsystem>,
) -> RuntimeResult<usize> {
    state(access)?.with_subsystem(audio.as_ptr() as usize, |subsystem| {
        Ok(subsystem.fidelity_active_sources().count())
    })
}

/// Returns the live sound identity and paused flag at a fidelity source index.
pub fn fidelity_active_source(
    access: CoreAccess<'_>,
    audio: std::ptr::NonNull<UALAudioSubsystem>,
    index: usize,
) -> RuntimeResult<Option<(usize, bool)>> {
    state(access)?.with_subsystem(audio.as_ptr() as usize, |subsystem| {
        Ok(subsystem.fidelity_active_sources().nth(index))
    })
}

fn config(audio: *mut UALAudioSubsystem, max_radius_multiplier: f32) -> Config {
    unsafe {
        Config {
            doppler_factor: ptr::addr_of!((*audio).doppler_factor).read_unaligned(),
            music_volume: ptr::addr_of!((*audio).music_volume).read_unaligned(),
            sound_volume: ptr::addr_of!((*audio).sound_volume).read_unaligned(),
            roll_off: ptr::addr_of!((*audio).roll_off).read_unaligned(),
            max_channels: ptr::addr_of!((*audio).max_channels).read_unaligned(),
            use_precache: ptr::addr_of!((*audio).use_precache).read_unaligned() != 0,
            max_radius_multiplier,
            use_eax: ptr::addr_of!((*audio).use_eax).read_unaligned() != 0,
            use_mmsystem: ptr::addr_of!((*audio).use_mmsystem).read_unaligned() != 0,
            reverse_stereo: ptr::addr_of!((*audio).reverse_stereo).read_unaligned() != 0,
        }
    }
}

fn with_audio<T>(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    operation: impl FnOnce(&mut Subsystem, &mut crate::audio_core_host::CoreHost<'_>) -> HostResult<T>,
) -> RuntimeResult<T> {
    if audio.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    state(access)?.with_subsystem(audio as usize, |subsystem| {
        let mut host = crate::audio_core_host::CoreHost::new(access);
        operation(subsystem, &mut host).map_err(|_| host.take_error("Audio subsystem operation"))
    })
}

/// Initializes the subsystem, then attaches the initial viewport in source order.
pub(crate) fn initialize(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    viewport: *mut UViewport,
) -> RuntimeResult<bool> {
    if audio.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    let owner = state(access)?;
    let was_initialized = owner.with_subsystem(audio as usize, |subsystem| Ok(subsystem.initialized))?;
    owner.ensure_stream_manager()?;
    let result = with_audio(access, audio, |subsystem, host| {
        let mut cfg = config(audio, host.audio_radius_multiplier());
        let initialized = subsystem.init(host, &mut cfg)?;
        unsafe { ptr::addr_of_mut!((*audio).roll_off).write_unaligned(cfg.roll_off); }
        Ok(initialized)
    });
    match result {
        Ok(true) => {
            if !was_initialized {
                owner.stream_users.set(owner.stream_users.get().saturating_add(1));
                set_sound_audio(access, audio)?;
            }
            set_viewport(access, audio, viewport)?;
            Ok(true)
        }
        Ok(false) => {
            owner.release_stream_manager_if_unused()?;
            Ok(false)
        }
        Err(error) => {
            let now_initialized = owner.with_subsystem(audio as usize, |subsystem| Ok(subsystem.initialized))?;
            if !was_initialized && now_initialized {
                owner.stream_users.set(owner.stream_users.get().saturating_add(1));
            } else {
                owner.release_stream_manager_if_unused()?;
            }
            Err(error)
        }
    }
}

pub(crate) fn set_viewport(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    viewport: *mut UViewport,
) -> RuntimeResult<()> {
    with_audio(access, audio, |subsystem, host| {
        let cfg = config(audio, host.audio_radius_multiplier());
        subsystem.set_viewport(host, &cfg, viewport as usize)?;
        unsafe { ptr::addr_of_mut!((*audio).viewport).write_unaligned(subsystem.viewport as *mut std::ffi::c_void); }
        Ok(())
    })
}

pub(crate) fn detach_viewport(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    viewport: *mut UViewport,
) -> RuntimeResult<()> {
    if audio.is_null() || viewport.is_null() {
        return Ok(());
    }
    with_audio(access, audio, |subsystem, host| {
        if subsystem.viewport == viewport as usize {
            let cfg = config(audio, host.audio_radius_multiplier());
            subsystem.set_viewport(host, &cfg, 0)?;
            unsafe { ptr::addr_of_mut!((*audio).viewport).write_unaligned(ptr::null_mut()); }
        }
        Ok(())
    })
}

pub(crate) fn shutdown(access: CoreAccess<'_>, audio: *mut UALAudioSubsystem) -> RuntimeResult<()> {
    if audio.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    let owner = state(access)?;
    let was_initialized = owner.with_subsystem(audio as usize, |subsystem| Ok(subsystem.initialized))?;
    if was_initialized {
        set_sound_audio(access, ptr::null_mut())?;
    }
    with_audio(access, audio, |subsystem, host| {
        let cfg = config(audio, host.audio_radius_multiplier());
        subsystem.destroy(host, &cfg)?;
        unsafe { ptr::addr_of_mut!((*audio).viewport).write_unaligned(subsystem.viewport as *mut std::ffi::c_void); }
        Ok(())
    })?;
    if was_initialized {
        owner.stream_users.set(owner.stream_users.get().saturating_sub(1));
        owner.release_stream_manager_if_unused()?;
    }
    Ok(())
}

pub(crate) fn update(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    frame: *const generated::FSceneNode,
) -> RuntimeResult<()> {
    with_audio(access, audio, |subsystem, host| {
        if subsystem.viewport == 0 {
            return Ok(());
        }
        let coords = unsafe { ptr::addr_of!((*frame).coords).read_unaligned() };
        let view = hp2_audio::update::UpdateView {
            view_location: [coords.origin.x, coords.origin.y, coords.origin.z],
            x_axis: [coords.x_axis.x, coords.x_axis.y, coords.x_axis.z],
            y_axis: [coords.y_axis.x, coords.y_axis.y, coords.y_axis.z],
            z_axis: [coords.z_axis.x, coords.z_axis.y, coords.z_axis.z],
        };
        let cfg = config(audio, host.audio_radius_multiplier());
        subsystem.update(host, &cfg, &view)
    })
}

pub(crate) fn play_sound(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    request: &hp2_audio::playback::PlayRequest,
) -> RuntimeResult<i32> {
    with_audio(access, audio, |subsystem, host| {
        let cfg = config(audio, host.audio_radius_multiplier());
        subsystem.play_sound(host, &cfg, request)
    })
}

pub(crate) fn stop_sound_by_actor(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    actor: usize,
    sound: usize,
) -> RuntimeResult<bool> {
    with_audio(access, audio, |subsystem, host| subsystem.stop_sound_by_actor(host, actor, sound))
}

pub(crate) fn stop_sound_by_id(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    actor: usize,
    id: i32,
    sound: usize,
    fade_out_time: f32,
) -> RuntimeResult<bool> {
    with_audio(access, audio, |subsystem, host| {
        subsystem.stop_sound_by_id(host, actor, id, sound, fade_out_time)
    })
}

pub(crate) fn modify_sound(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    actor: usize,
    id: i32,
    sound: usize,
    parameter: u8,
    value: f32,
) -> RuntimeResult<bool> {
    with_audio(access, audio, |subsystem, host| {
        subsystem.modify_sound(host, actor, id, sound, parameter, value)
    })
}

pub(crate) fn play_music(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    song: &str,
    fade_in_time: f32,
) -> RuntimeResult<i32> {
    with_audio(access, audio, |subsystem, host| {
        let cfg = config(audio, host.audio_radius_multiplier());
        subsystem.play_music(host, &cfg, song, fade_in_time)
    })
}

pub(crate) fn stop_music(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    song_handle: i32,
    fade_out_time: f32,
) -> RuntimeResult<bool> {
    with_audio(access, audio, |subsystem, host| subsystem.stop_music(host, song_handle, fade_out_time))
}

pub(crate) fn stop_all_music(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    fade_out_time: f32,
) -> RuntimeResult<()> {
    with_audio(access, audio, |subsystem, host| subsystem.stop_all_music(host, fade_out_time))
}

pub(crate) fn pause_sounds(access: CoreAccess<'_>, audio: *mut UALAudioSubsystem) -> RuntimeResult<()> {
    with_audio(access, audio, |subsystem, host| {
        subsystem.pause_sounds(host);
        Ok(())
    })
}

pub(crate) fn unpause_sounds(access: CoreAccess<'_>, audio: *mut UALAudioSubsystem) -> RuntimeResult<()> {
    with_audio(access, audio, |subsystem, host| {
        subsystem.unpause_sounds(host);
        Ok(())
    })
}

struct AudioExecContext<'a> {
    audio: *mut UALAudioSubsystem,
    subsystem: *mut Subsystem,
    host: *mut crate::audio_core_host::CoreHost<'a>,
}

unsafe extern "C" fn invoke_audio_exec(
    opaque: *mut std::ffi::c_void,
    leaf: u32,
    text: *const std::ffi::c_void,
) -> i32 {
    use hp2_core::ansi::Tchar;
    let Some(context) = (unsafe { opaque.cast::<AudioExecContext<'_>>().as_mut() }) else {
        return 1;
    };
    // SAFETY: console::exec invokes the callback synchronously while the owner
    // lease, subsystem, host, and optional text tail all remain live.
    let subsystem = unsafe { &mut *context.subsystem };
    let host = unsafe { &mut *context.host };
    let result = match leaf {
        value if value == Hp2AudioConsoleLeaf::PauseSounds as u32 => {
            subsystem.exec(host, ExecCommand::PauseSounds)
        }
        value if value == Hp2AudioConsoleLeaf::UnpauseSounds as u32 => {
            subsystem.exec(host, ExecCommand::UnpauseSounds)
        }
        value if value == Hp2AudioConsoleLeaf::RestartSounds as u32 => {
            subsystem.exec(host, ExecCommand::RestartSounds)
        }
        value if value == Hp2AudioConsoleLeaf::PlayingSoundStats as u32 => {
            subsystem.exec(host, ExecCommand::PlayingSoundStats)
        }
        value if value == Hp2AudioConsoleLeaf::SetWeaponRadius as u32
            || value == Hp2AudioConsoleLeaf::SetRollOff as u32 =>
        {
            if text.is_null() {
                return 1;
            }
            // `appAtof` accepts a numeric prefix and returns zero for a
            // nonnumeric prefix. The parsed command tail is NUL-terminated.
            let mut units = Vec::new();
            let mut cursor = text.cast::<Tchar>();
            // SAFETY: `text` is a command-tail pointer retained by console::exec.
            unsafe {
                while *cursor != 0 {
                    units.push(*cursor);
                    cursor = cursor.add(1);
                }
            }
            let parsed = atof_prefix(&units);
            if value == Hp2AudioConsoleLeaf::SetWeaponRadius as u32 {
                // SAFETY: shared Core/native global; command runs on the engine thread.
                unsafe { hp2_core::audio_globals::GAudioDefaultRadius = parsed; }
            } else {
                // SAFETY: `audio` is the live canonical subsystem record for this call.
                unsafe { ptr::addr_of_mut!((*context.audio).roll_off).write_unaligned(parsed); }
            }
            Ok(())
        }
        value if value == Hp2AudioConsoleLeaf::StopSounds as u32 => {
            subsystem.stop_sound_by_actor(host, 0, 0).map(|_| ())
        }
        _ => return 1,
    };
    if result.is_ok() { 0 } else { 1 }
}

fn atof_prefix(units: &[hp2_core::ansi::Tchar]) -> f32 {
    let mut bytes = Vec::with_capacity(units.len() + 1);
    bytes.extend(units.iter().take_while(|unit| **unit != 0).map(|unit| {
        u8::try_from(*unit).unwrap_or(b'?')
    }));
    bytes.push(0);
    // SAFETY: `bytes` is NUL-terminated and `atof` does not retain it.
    unsafe { libc::atof(bytes.as_ptr().cast()) as f32 }
}

/// Executes `UALAudioSubsystem::Exec` using Core-owned records and Ar output.
///
/// # Safety
/// `audio` must be a live canonical ALAudioSubsystem and `cmd` must be
/// NUL-terminated UTF-32 storage valid through the synchronous dispatch.
pub(crate) unsafe fn exec(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    cmd: *const u32,
    output: &mut crate::exec_output::ExecOutput<'_>,
) -> RuntimeResult<bool> {
    if audio.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    state(access)?.with_subsystem(audio as usize, |subsystem| {
        let mut host = crate::audio_core_host::CoreHost::with_output(access, output);
        let mut context = AudioExecContext {
            audio,
            subsystem,
            host: (&mut host as *mut crate::audio_core_host::CoreHost<'_>).cast(),
        };
        let services = Hp2AudioConsoleServices {
            opaque: (&mut context as *mut AudioExecContext<'_>).cast(),
            invoke: invoke_audio_exec,
        };
        // SAFETY: caller guarantees a live terminated command; service context
        // and all resources remain live through this synchronous parser call.
        let handled = unsafe { hp2_audio::console::exec(&services, cmd.cast::<hp2_core::ansi::Tchar>()) }
            .map_err(|_| host.take_error("Audio Exec operation"))?;
        Ok(handled)
    })
}

pub(crate) fn post_render(
    _: CoreAccess<'_>,
    _: *mut UALAudioSubsystem,
    _: *mut generated::FSceneNode,
) -> RuntimeResult<()> {
    Ok(())
}

pub(crate) fn register_sound(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
    sound: *mut hp2_abi::engine::generated::USound,
) -> RuntimeResult<bool> {
    with_audio(access, audio, |subsystem, host| subsystem.register_sound(host, sound as usize))
}

pub(crate) fn unregister_sound(
    access: CoreAccess<'_>,
    receiver: std::ptr::NonNull<UObject>,
    sound: std::ptr::NonNull<hp2_abi::engine::generated::USound>,
) -> RuntimeResult<()> {
    with_audio(
        access,
        receiver.as_ptr().cast(),
        |subsystem, host| subsystem.unregister_sound(host, sound.as_ptr() as usize, -1),
    )
}

pub(crate) fn note_destroy(
    access: CoreAccess<'_>,
    receiver: std::ptr::NonNull<UObject>,
    actor: std::ptr::NonNull<hp2_abi::engine::generated::AActor>,
) -> RuntimeResult<()> {
    with_audio(access, receiver.as_ptr().cast(), |subsystem, host| {
        subsystem.note_destroy(host, actor.as_ptr() as usize)
    })
}

pub(crate) fn is_initialized(access: CoreAccess<'_>, audio: *mut UALAudioSubsystem) -> RuntimeResult<bool> {
    if audio.is_null() {
        return Err(RuntimeError::new("Null canonical ALAudioSubsystem"));
    }
    state(access)?.with_subsystem(audio as usize, |subsystem| Ok(subsystem.initialized))
}

pub(crate) fn shutdown_after_error(
    access: CoreAccess<'_>,
    audio: *mut UALAudioSubsystem,
) -> RuntimeResult<()> {
    if is_initialized(access, audio)? {
        set_sound_audio(access, ptr::null_mut())?;
    }
    with_audio(access, audio, |subsystem, host| subsystem.shutdown_after_error(host))
}
impl AudioOwnerState {
    fn ensure_stream_manager(&self) -> RuntimeResult<()> {
        if !hp2_audio::file_stream::hp2_rs_stream_manager().is_null() {
            return Ok(());
        }
        let mut manager = ptr::null_mut();
        let status = unsafe {
            hp2_audio::file_stream::hp2_rs_stream_init(
                hp2_audio::file_stream::HP2_STREAM_CAPACITY as i32,
                &mut manager,
            )
        };
        if status != 0 || manager.is_null() {
            return Err(RuntimeError::new("Unable to initialize canonical audio file-stream manager"));
        }
        self.stream_manager_owned.set(true);
        Ok(())
    }

    fn release_stream_manager_if_unused(&self) -> RuntimeResult<()> {
        if !self.stream_manager_owned.get() || self.stream_users.get() != 0 {
            return Ok(());
        }
        let status = unsafe { hp2_audio::file_stream::hp2_rs_stream_shutdown() };
        if status != 0 {
            return Err(RuntimeError::new("Unable to shut down canonical audio file-stream manager"));
        }
        self.stream_manager_owned.set(false);
        Ok(())
    }

    /// Registers a newly constructed canonical audio subsystem.
    pub(crate) fn register(&self, object: usize, last_time: i64) -> RuntimeResult<()> {
        if object == 0 {
            return Err(RuntimeError::new("Null canonical audio subsystem"));
        }
        let mut resources = self.resources.borrow_mut();
        if resources.contains_key(&object) {
            return Err(RuntimeError::new("Canonical audio subsystem is already registered"));
        }
        resources.insert(object, Rc::new(AudioRuntime {
            busy: Cell::new(false),
            subsystem: RefCell::new(Some(Subsystem::new(last_time))),
        }));
        Ok(())
    }

    /// Runs an audio operation without retaining owner borrows across callbacks.
    ///
    /// Reentrant operations on the same subsystem fail explicitly while the
    /// active operation retains exclusive ownership of its state.
    pub(crate) fn with_subsystem<T>(
        &self,
        object: usize,
        operation: impl FnOnce(&mut Subsystem) -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        let runtime = self.resources.borrow().get(&object).cloned()
            .ok_or_else(|| RuntimeError::new("Canonical audio subsystem is not registered"))?;
        if runtime.busy.replace(true) {
            return Err(RuntimeError::new("Reentrant canonical audio subsystem operation"));
        }
        let subsystem = runtime.subsystem.borrow_mut().take();
        let Some(subsystem) = subsystem else {
            runtime.busy.set(false);
            return Err(RuntimeError::new("Canonical audio subsystem state is unavailable"));
        };
        let mut lease = AudioRuntimeLease { runtime, subsystem: Some(subsystem) };
        let subsystem = lease.subsystem.as_mut()
            .ok_or_else(|| RuntimeError::new("Canonical audio runtime lease is empty"))?;
        operation(subsystem)
    }

    /// Unregisters a subsystem after source-ordered explicit SDK teardown.
    pub(crate) fn unregister(&self, object: usize) -> RuntimeResult<()> {
        let mut resources = self.resources.borrow_mut();
        let Some(runtime) = resources.get(&object) else {
            return Err(RuntimeError::new("Canonical audio subsystem is not registered"));
        };
        if runtime.busy.get() {
            return Err(RuntimeError::new("Cannot unregister an active canonical audio subsystem"));
        }
        resources.remove(&object);
        Ok(())
    }
}

struct AudioRuntimeLease {
    runtime: Rc<AudioRuntime>,
    subsystem: Option<Subsystem>,
}

impl Drop for AudioRuntimeLease {
    fn drop(&mut self) {
        let subsystem = self.subsystem.take();
        *self.runtime.subsystem.borrow_mut() = subsystem;
        self.runtime.busy.set(false);
    }
}
