//! Rust port of `HarryPotter2/Unreal/SDLDrv/` -- the SDL2-backed
//! implementation of `UClient`/`UViewport`, registered as
//! `SDLDrv.SDLClient`/`SDLDrv.SDLViewport`.
//!
//! **Scope of this crate today: two pure decision/mapping seams, not
//! the live `USDLClient`/`USDLViewport` classes themselves.**
//! `USDLClient : UClient, FNotifyHook` and `USDLViewport : UViewport`
//! are UE1 `UObject`-derived classes (`DECLARE_CLASS`,
//! `DECLARE_WITHIN(USDLClient)`) with large virtual interfaces (19 and
//! 25 methods respectively) driven directly by the engine's viewport/
//! client machinery. Porting those classes (the plan's "C++ stub
//! forwards every virtual to `hp2_rs_platform_*`" design) needs the
//! same kind of live-engine-object FFI boundary `hp2-audio` is
//! blocked on -- see that crate's own module doc for the full
//! reasoning, which applies here unchanged. That is real, substantial
//! work, scoped but not started.
//!
//! What *is* safely portable today, and ported here:
//!
//! - `HP2MouseCapturePolicy.h` (`SDLDrv/Inc/`, 106 lines) -- by its
//!   own header comment, "deliberately free of SDL and engine
//!   includes": a pure decision table (`OnLoss`/`OnRestore`) the real
//!   `USDLViewport::UpdateInput`'s `SDL_WINDOWEVENT` handler
//!   consults, with its own existing table-driven C++ test
//!   (`Tests/InputContractTests.cpp`'s `TestMouseCapturePolicy`,
//!   registered as the `mouse_capture_policy` ctest).
//! - `HP2InputEvents.h` (`SDLDrv/Inc/`, 291 lines) -- raw-SDL ->
//!   normalized-event mapping functions. The real header is itself
//!   "PREP ONLY": not yet wired into the real `USDLViewport` event
//!   pump. That pump rewrite is real future work for this crate, not
//!   dead code to defer indefinitely -- because the FFI boundary
//!   stays C++-calls-Rust-only until Phase 5, the real pump can only
//!   work as "C++ calls `hp2_rs_sdl_poll`, Rust drains
//!   `SDL_PollEvent`, returns a batch of normalized events, C++
//!   dispatches them to `CauseInputEvent`/`Engine->Key`", and
//!   `FHP2InputEvent` (ported here as `HP2InputEvent`) is exactly
//!   that normalized record shape. Oracle:
//!   `Tests/InputContractTests.cpp`'s `TestInputEventMapping`,
//!   registered as the `input_event_mapping` ctest.
//!
//! Not yet FFI-wired or called from C++: an additive, isolated crate,
//! same as every other port crate's first commit in this workspace.

pub mod input_event;
pub mod mouse_capture;
