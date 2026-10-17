//! Rust port of `HarryPotter2/Unreal/SDLDrv/` -- the SDL2-backed
//! implementation of `UClient`/`UViewport`, registered as
//! `SDLDrv.SDLClient`/`SDLDrv.SDLViewport`.
//!
//! **Scope of this crate today: one pure decision seam.**
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
//! `HP2MouseCapturePolicy.h` (`SDLDrv/Inc/`, 106 lines) -- by its own
//! header comment, "deliberately free of SDL and engine includes": a
//! pure decision table (`OnLoss`/`OnRestore`) the real
//! `USDLViewport::UpdateInput`'s `SDL_WINDOWEVENT` handler consults,
//! with its own existing table-driven C++ test
//! (`Tests/InputContractTests.cpp`'s `TestMouseCapturePolicy`,
//! registered as the `mouse_capture_policy` ctest, one of Phase 2's
//! named gates) already pinning its exact behavior.
//!
//! Not yet FFI-wired or called from C++: an additive, isolated crate,
//! same as every other port crate's first commit in this workspace.

pub mod mouse_capture;
