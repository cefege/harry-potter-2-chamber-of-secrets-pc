//! Rust port of the ALAudio subsystem (`HarryPotter2/Unreal/ALAudio/Src/
//! ALAudioSubsystem.cpp`), the OpenAL-backed implementation of
//! `UAudioSubsystem` registered as `ALAudio.ALAudioSubsystem`.
//!
//! **Rust owns the subsystem logic.** [`subsystem::Subsystem`] holds the
//! `Sources`, `Buffers`, and `Streams` state, and the modules implement the
//! behavior that used to live in the C++ class body:
//!
//! - `subsystem`: `Init`/`Destroy` (OpenAL device and context bring-up and
//!   teardown, the `alError` checks and log order), `RegisterSound` /
//!   `UnregisterSound` for ogg, XA, and WAV, `SetViewport`, and the stream
//!   record bookkeeping.
//! - `playback`: `PlaySound`, `StopSound` (both overloads), `StopSoundSource`,
//!   `OldStopSound`, `ModifySound`, `NoteDestroy`, and the occlusion-scaled
//!   reference distance used by PlaySound and ModifySound.
//! - `update`: `Update`'s control flow (pause/unpause transitions, finished
//!   source underrun and check(0) handling, `HandleAmbience`, `UpdateAmbience`,
//!   the per-source `UpdateSounds` pass, fade and occlusion gating, and the
//!   listener position, orientation, and velocity writes).
//! - `music`: `PlayMusic`, `StopMusic`, `StopAllMusic`, `PauseSounds`, and
//!   `UnpauseSounds`.
//! - `exec`: the Exec command bodies (PAUSESOUNDS, UNPAUSESOUNDS, RESTARTSOUNDS,
//!   PLAYINGSOUNDSTATS).
//! - `stream`: the streaming buffer state machine (`UpdateStreamingSounds`),
//!   with the OpenAL and `GFileStream` calls passed in as fallible callbacks.
//! - `lipsync`: lip-sync timing gate and the mouth channel handle (the former
//!   `FLipSyncAnimMgr`).
//! - Pure helpers: `vec3` (UE1 `FVector`, bit-exact), `listener` (projection,
//!   listener and source velocity gates, delta-time clamp), `fade`, `radius`,
//!   `priority` (`SoundPriority`), `ambient` (`HandleAmbience` admission and
//!   flags), `sort` (UE1 `Sort`), `al` (OpenAL and ALC enum values, plus
//!   `alError`).
//!
//! **Still C++, and why.** The C++ shell is `UALAudioSubsystem`, a UE1
//! `UObject` class (`DECLARE_CLASS(..., CLASS_Config, ALAudio)`). It keeps:
//!
//! - The UObject shell: class declaration, `StaticConstructor` property
//!   registration, config properties (`UseEAX`, `CompatibilityMode`,
//!   `ReverseStereo`, `MaxChannels`, and the others, so the `.ini` keys do not
//!   change), `USound::Audio = this` ownership, and the owned `Box<Subsystem>`.
//! - The OpenAL trampolines: each `alc*`/`al*` call is a thin `extern "C"`
//!   thunk, reached from Rust through the host table below.
//! - Engine-global services, which must stay in C++: `GFileStream` chunk
//!   requests, `TObjectIterator` over `USound`, `AActor`/`ULevel`/`UViewport`
//!   accessors, `IsAudibleAt`, `PointRegion`, `CalculateRadiusMultiplier`,
//!   `debugf`, `appSeconds`, the `Ar` output device, and `FTime` (its tick
//!   count is private, so the shell memcpys it into an `i64`).
//! - Exception containment: engine calls can throw. Each trampoline that calls
//!   an engine function that can throw (USound, AActor, ULevel, GFileStream,
//!   the log and error sinks) wraps its body in `try`/`catch (...)`, stores
//!   `std::current_exception()` in a per-call context, and returns failure.
//!   Rust returns `Err(HostFailed)` and never holds a borrow across an engine
//!   call. The C++ export shell rethrows the stored exception once the Rust
//!   export has returned. The OpenAL/ALC thunks and `appSeconds` are direct
//!   calls and do not throw.
//! - Exec parsing: `ParseCommand` dispatch, `ASTAT`, `WEAPONRADIUS`
//!   (`GAudioDefaultRadius` is an engine global), and `ROLLOFF` (sets the
//!   `RollOff` UObject property, then calls the Rust StopSound overload 1).
//!
//! Host calls go through [`host::Host`]. The C++ shell implements it with one
//! function table per call (`hp2_rs_audio_*` exports in `crates/hp2-ffi`).

pub mod al;
pub mod ambient;
pub mod exec;
pub mod fade;
pub mod host;
pub mod lipsync;
pub mod listener;
pub mod music;
pub mod playback;
pub mod priority;
pub mod radius;
pub mod sort;
pub mod stream;
pub mod subsystem;
pub mod update;
pub mod vec3;
