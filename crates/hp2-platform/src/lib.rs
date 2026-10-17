//! Rust port of `HarryPotter2/Unreal/SDLDrv/` -- the SDL2-backed
//! implementation of `UClient`/`UViewport`, registered as
//! `SDLDrv.SDLClient`/`SDLDrv.SDLViewport`.
//!
//! **Scope of this crate today: two pure mapping/decision modules plus
//! one live FFI-wired slice, not the `USDLClient`/`USDLViewport`
//! classes themselves.** `USDLClient : UClient, FNotifyHook` and
//! `USDLViewport : UViewport` are UE1 `UObject`-derived classes
//! (`DECLARE_CLASS`, `DECLARE_WITHIN(USDLClient)`) with large virtual
//! interfaces (19 and 25 methods respectively) driven directly by the
//! engine's viewport/client machinery. Porting those classes (the
//! plan's "C++ stub forwards every virtual to `hp2_rs_platform_*`"
//! design) needs the same kind of live-engine-object FFI boundary
//! `hp2-audio` is blocked on -- see that crate's own module doc.
//!
//! - `mouse_capture`: `HP2MouseCapturePolicy.h`'s pure decision table
//!   (`OnLoss`/`OnRestore`), consulted by `USDLViewport::UpdateInput`'s
//!   `SDL_WINDOWEVENT` handler. Oracle:
//!   `Tests/InputContractTests.cpp`'s `TestMouseCapturePolicy`
//!   (`mouse_capture_policy` ctest). Not yet FFI-wired.
//! - `input_event`: `HP2InputEvents.h`'s raw-SDL -> normalized-event
//!   mapping functions. `USDLViewport::UpdateInput`
//!   (`SDLViewport.cpp:318-480`) already calls these at five live call
//!   sites -- the constructor fills `KeysymMap` from `HP2MapKeysym`
//!   (`:84-85`), and the event loop calls `HP2MakeKeyEvent` (`:345`),
//!   `HP2MakeTextEvent` (`:352`), `HP2MakeMouseButtonEvent` (`:370`)
//!   and `HP2MakeWheelEvents` (`:378`), each immediately followed by
//!   `CauseInputEvent`/`Engine->Key`. The cutover is switching those
//!   five call sites to `hp2_rs_*` equivalents one at a time -- each
//!   call keeps returning one `HP2InputEvent` (or up to three, for the
//!   wheel case), C++ keeps doing the `CauseInputEvent`/`Engine->Key`
//!   dispatch immediately after, same shape as today. Oracle:
//!   `Tests/InputContractTests.cpp`'s `TestInputEventMapping`
//!   (`input_event_mapping` ctest). Not yet FFI-wired.
//! - `clipboard`: `USDLClient::GetClipboardText`/`SetClipboardText`.
//!   FFI-wired; the real C++ caller already calls through it. Ctest:
//!   `clipboard_round_trip` (`Tests/PlatformClipboardTests.cpp`).
//!
//! **`USDLViewport` and Phase 3 (the wgpu renderer) are not cleanly
//! separable in the plan's stated order.** `OpenWindow` calls
//! `TryRenderDevice` directly (`SDLViewport.cpp:304-309`);
//! `TryRenderDevice` constructs the render device through UE1's
//! class-loading reflection (`StaticLoadClass`,
//! `ConstructObject<URenderDevice>`, `:669-722`) and stores it in the
//! inherited `RenDev` field; `Lock`/`Unlock`/`ResizeViewport` all read
//! or call through `RenDev` (`:160-212`). None of that can move to
//! Rust before a render device exists on the Rust side to move it to.
//!
//! What can port independent of Phase 3, once the live-UObject FFI
//! boundary is designed: window creation/chrome/cursor (`OpenWindow`'s
//! `SDL_CreateWindow`/GL-attribute half, `UpdateWindowFrame`,
//! `SetTitleBar`, `SetModeCursor`, `ToggleFullscreen`/`EndFullscreen`'s
//! SDL half), the rest of the `UpdateInput` event pump (joystick axis/
//! button/hat branches, pure given explicit `JoystickDeadZone`/
//! `ScaleJBX`/`ScaleJBY`/`JoyHatFoldsToButtons` inputs; ungrabbed
//! `SDL_MOUSEMOTION` dispatches straight to `Engine->MousePosition`
//! with no `HP2Make*Event` call, so it is not part of `input_event`'s
//! scope), mouse grab/capture state (`UpdateMouseGrabState`,
//! `SetMouseCapture`, `GetMouseState`), joystick open/close
//! (`USDLClient::OpenJoystick`/`CloseJoystick`), and macOS
//! `TextToSpeech` (`fork`/`execl("/usr/bin/say")`, `:770-813`).

pub mod clipboard;
pub mod input_event;
pub mod mouse_capture;
