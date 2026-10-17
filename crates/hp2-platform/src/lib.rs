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
//! **Scoping finding from reading all of `SDLClient.cpp` (318 lines)
//! and `SDLViewport.cpp` (817 lines) in full:** Phase 2 (this crate)
//! and Phase 3 (the wgpu renderer) are not cleanly separable in the
//! order the plan states them for `USDLViewport` specifically.
//! `OpenWindow` calls `TryRenderDevice` directly (`SDLViewport.cpp:
//! 304-309`); `TryRenderDevice` constructs the render device through
//! UE1's class-loading reflection (`StaticLoadClass`,
//! `ConstructObject<URenderDevice>`, `SDLViewport.cpp:669-722`) and
//! stores it in the inherited `RenDev` field; `Lock`/`Unlock`/
//! `ResizeViewport` all read or call through `RenDev`
//! (`SDLViewport.cpp:160-212`). None of that can move to Rust before
//! a render device exists on the Rust side to move it to -- a C++
//! stub that "forwards every virtual to `hp2_rs_platform_*`" for
//! `USDLViewport` specifically would need Phase 3's design in hand
//! for at least those five methods.
//!
//! What *can* port independent of Phase 3, once the live-UObject FFI
//! boundary below is designed: window creation/chrome/cursor
//! (`OpenWindow`'s `SDL_CreateWindow`/GL-attribute half,
//! `UpdateWindowFrame`, `SetTitleBar`, `SetModeCursor`,
//! `ToggleFullscreen`/`EndFullscreen`'s SDL half), the full
//! `UpdateInput` event pump (already has its mapping logic ported --
//! `input_event`/`mouse_capture` above -- missing only the joystick
//! axis/button/hat branches, which are pure given explicit
//! `JoystickDeadZone`/`ScaleJBX`/`ScaleJBY`/`JoyHatFoldsToButtons`
//! inputs), mouse grab/capture state (`UpdateMouseGrabState`,
//! `SetMouseCapture`, `GetMouseState`), clipboard
//! (`USDLClient::GetClipboardText`/`SetClipboardText`), joystick
//! open/close (`USDLClient::OpenJoystick`/`CloseJoystick`), and
//! macOS `TextToSpeech` (`fork`/`execl("/usr/bin/say")`,
//! `SDLViewport.cpp:770-813`).
//!
//! The pump's output can't be a single flat event list: distinct
//! engine entry points are called per event kind --
//! `Engine->InputEvent` for ordinary key/button/axis/wheel/joystick
//! events (`CauseInputEvent`, `SDLViewport.cpp:535-542`),
//! `Engine->MousePosition` for ungrabbed `SDL_MOUSEMOTION` (raw X/Y,
//! not a delta, `SDLViewport.cpp:358-366`), and `Engine->Key` for
//! `SDL_TEXTINPUT` (`SDLViewport.cpp:350-357`) -- `hp2_rs_sdl_poll`'s
//! batch needs to carry which dispatch each entry needs, not just
//! `(Key, Action, Delta)`.
//!
//! `clipboard` is the exception: FFI-wired, real `USDLClient` caller
//! switched to it (`GetClipboardText`/`SetClipboardText`). Everything
//! else above is additive and not yet called from C++, same as every
//! other port crate's first commit in this workspace.

pub mod clipboard;
pub mod input_event;
pub mod mouse_capture;
