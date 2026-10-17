//! Rust port of `HarryPotter2/Unreal/ALAudio/Src/ALAudioSubsystem.cpp`
//! (2304 lines) -- the OpenAL-Soft-backed implementation of
//! `UAudioSubsystem`, registered as `ALAudio.ALAudioSubsystem`.
//!
//! **Scope of this crate today: one function.** `UALAudioSubsystem`
//! is a UE1 `UObject`-derived class (`DECLARE_CLASS(UALAudioSubsystem,
//! UAudioSubsystem, CLASS_Config, ALAudio)`) whose state and virtual
//! interface are deeply coupled to the live engine object graph --
//! `USound::Audio = this`, `TObjectIterator<USound>`, per-tick
//! `AActor*`/`USound*` access for positional audio, and UE1's property
//! reflection system for config persistence (`StaticConstructor`'s
//! `UBoolProperty`/`UFloatProperty` registrations). Porting the class
//! itself (matching the plan's "C++ stub forwards every virtual to
//! `hp2_rs_audio_*`" design) requires a kind of FFI boundary nothing
//! in this workspace has needed yet: Rust holding opaque handles to
//! live `AActor*`/`USound*`/`UViewport*` pointers, with new C++-side
//! accessor exports for whatever engine state each virtual needs
//! (there is no existing precedent for this in the codebase --
//! `hp2_rs_vorbis_*`, the one Rust call this file already makes, only
//! ever operates on byte streams, never on an engine object). That
//! design is real, substantial, multi-session work, scoped but not
//! started.
//!
//! What *is* safely portable today, and ported here: `ALSource` fields
//! `Sound`/`Id`/`Priority` for `FindLeastPriorityNonTalkSource`
//! (`ALAudioSubsystem.cpp:2085-2101`) only depend on a `Sound`
//! null-check, the slot bits of `Id`, and `Priority` -- genuinely pure
//! data, no object graph traversal, a `static` C++ method already
//! called with an explicit `TArray<ALSource>` parameter rather than
//! reading `this->Sources` directly (unlike its neighbor
//! `FindLeastImportantSound`, which does read `this->Sources` and
//! would need its own signature changed to port the same way -- left
//! alone for now, no caller update bundled into this slice).
//!
//! FFI-wired via `hp2_rs_audio_find_least_priority_non_talk_source`
//! (`crates/hp2-ffi`); `UALAudioSubsystem::FindLeastPriorityNonTalkSource`'s
//! body now forwards to it, builds the `FfiAudioSourceSlot` array from
//! its own `TArray<ALSource>` immediately before the call. The real
//! C++ oracle, `TestTalkRegistrationEvictionPolicy`
//! (`Tests/AudioTests.cpp:612-636`, part of the `audio_lifecycle`
//! ctest), exercises this through the real caller path and passes
//! against the Rust implementation.

pub mod priority;
