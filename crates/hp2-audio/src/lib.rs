//! Rust port of `HarryPotter2/Unreal/ALAudio/Src/ALAudioSubsystem.cpp`
//! (2261 lines) -- the OpenAL-Soft-backed implementation of
//! `UAudioSubsystem`, registered as `ALAudio.ALAudioSubsystem`.
//!
//! **Scope of this crate today:** the `priority` functions (three, below),
//! `ambient` (the `HandleAmbience` admission, flags, source-id, and
//! `UpdateAmbience` stop predicates, plus `sort`, a faithful port of the UE1
//! `Sort` template that `HandleAmbience` uses), and `stream`, the
//! streaming-buffer state machine (`UpdateStreamingSounds`,
//! `ALAudioSubsystem.cpp:1495-1597`) with OpenAL and `GFileStream` calls
//! passed in as closures. Device init, voice management, attenuation, and
//! music are still C++. `UALAudioSubsystem`
//! is a UE1 `UObject`-derived class (`DECLARE_CLASS(UALAudioSubsystem,
//! UAudioSubsystem, CLASS_Config, ALAudio)`) whose state and virtual
//! interface are deeply coupled to the live engine object graph --
//! `USound::Audio = this`, `TObjectIterator<USound>`, per-tick
//! `AActor*`/`USound*` access for positional audio, and UE1's property
//! reflection system for config persistence (`StaticConstructor`'s
//! `UBoolProperty`/`UFloatProperty` registrations). Porting the class
//! itself (matching the plan's "C++ stub forwards every virtual to
//! `hp2_rs_audio_*`" design) requires a kind of FFI boundary this
//! crate has only just started establishing: Rust holding opaque
//! identities for live `USound*` pointers (see
//! `find_least_important_sound` below, its first real use), but not
//! yet opaque identities for `AActor*`/`UViewport*`, and not yet
//! holding any handle *across calls* (every function here is still
//! stateless, called fresh each time with plain data). The full class
//! port is still real, substantial, multi-session work, scoped but
//! not started.
//!
//! **First: `FindLeastPriorityNonTalkSource`**
//! (`ALAudioSubsystem.cpp:2085-2101`). `ALSource` fields `Sound`/
//! `Id`/`Priority` only depend on a `Sound` null-check, the slot bits
//! of `Id`, and `Priority` -- genuinely pure data, no object graph
//! traversal, a `static` C++ method already called with an explicit
//! `TArray<ALSource>` parameter rather than reading `this->Sources`
//! directly. FFI-wired via
//! `hp2_rs_audio_find_least_priority_non_talk_source`
//! (`crates/hp2-ffi`); `FindLeastPriorityNonTalkSource`'s body now
//! forwards to it, builds the `FfiAudioSourceSlot` array from its own
//! `TArray<ALSource>` immediately before the call. The real C++
//! oracle, `TestTalkRegistrationEvictionPolicy`
//! (`Tests/AudioTests.cpp:612-636`, part of the `audio_lifecycle`
//! ctest), exercises this through the real caller path and passes
//! against the Rust implementation.
//!
//! **Second: `SoundPriority`'s pure math half**
//! (`ALAudioSubsystem.cpp:1946-1961`) -- given `Volume`/`Radius`/
//! `Flags` and the already-computed `FDistSquared(Location,
//! ViewTarget->Location)`, compute the priority score. C++ still
//! resolves `Viewport->Actor->ViewTarget`/`->Actor` and calls
//! `FDistSquared` itself (live `AActor`/`FVector` state), then passes
//! the single resulting float across. FFI-wired via
//! `hp2_rs_audio_sound_priority`; `SoundPriority`'s body now computes
//! only `DistSquared` before forwarding. `SoundPriority` had five
//! call sites when this slice landed; `FindLeastImportantSound`'s
//! own call (`:2134`) is gone now that its C++ body was replaced
//! entirely by a Rust forward (see "Third" below) -- it calls
//! `sound_priority` directly, not through `SoundPriority`'s C++ body.
//! Four C++ call sites remain: `:808` in `PlaySound`, `:1404`/`:1424`
//! gathering ambient sounds, `:1703` in `Update` for every active
//! source on every tick -- this Rust path still runs on every tick
//! for every active sound. No existing C++ oracle for any of them;
//! `audio_lifecycle` only exercises this function directly via
//! `TestSoundPriorityFfiBoundary`, covered by `priority.rs`'s own
//! Rust unit tests otherwise. `Update`/`PlaySound`'s runtime call
//! paths are not exercised by any automated check.
//!
//! **Third: `FindLeastImportantSound`** (`ALAudioSubsystem.cpp:
//! 2105-2156`), called from `PlaySound` (`:758`, `:842`) when a
//! new sound needs a source and every slot is full. Unlike the first
//! two, this one *does* read `this->Sources`/the live `USound*`
//! pointers directly in the real C++ -- the first function in this
//! crate needing a live-object FFI pattern rather than pure data
//! alone. Resolved as: `Sound` crosses as `sound: usize`, the pointer
//! bits with no dereference ability on the Rust side, only equality
//! comparison (matching exactly what the real C++'s `TMap<USound*,
//! INT>`/`pMaxCountSound` pointer identity and truthiness checks
//! need) -- `0` doubles as both "never assigned" and "a null `Sound`"
//! the same way a null `USound*` does in C++. `WorkingFlags` is read
//! once per source up front in C++ (`Source.Sound ? ->WorkingFlags :
//! 0`) rather than only for the eventually-selected candidates like
//! the original's lazier gating -- a deliberate simplification
//! (same deterministic result, bounded extra `FDistSquared`/pointer
//! read cost for a typically-small `Sources` array) to keep this to
//! one FFI round trip instead of two. FFI-wired via
//! `hp2_rs_audio_find_least_important_sound`; reuses `sound_priority`
//! internally for the second-pass comparison. No existing C++ oracle;
//! covered by `priority.rs`'s own Rust unit tests (including the
//! null-identity and "first sound to strictly exceed the running
//! max" tie-break edge cases) plus an FFI-boundary check added to
//! `Tests/AudioTests.cpp` (`TestFindLeastImportantSoundFfiBoundary`,
//! same `audio_lifecycle` ctest).

pub mod ambient;

pub mod priority;

pub mod stream;

pub mod sort;
