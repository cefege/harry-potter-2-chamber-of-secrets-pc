//! Rust port of `HarryPotter2/Unreal/SDLDrv/` -- the SDL2-backed
//! implementation of `UClient`/`UViewport`, registered as
//! `SDLDrv.SDLClient`/`SDLDrv.SDLViewport`.
//!
//! **Scope of this crate today: four FFI-wired cutovers, not the
//! `USDLClient`/`USDLViewport` classes themselves.** `USDLClient :
//! UClient, FNotifyHook` and `USDLViewport : UViewport` are UE1
//! `UObject`-derived classes (`DECLARE_CLASS`,
//! `DECLARE_WITHIN(USDLClient)`) with large virtual interfaces (19 and
//! 25 methods respectively) driven directly by the engine's viewport/
//! client machinery. Porting those classes (the plan's "C++ stub
//! forwards every virtual to `hp2_rs_platform_*`" design) needs the
//! same kind of live-engine-object FFI boundary `hp2-audio` is
//! blocked on -- see that crate's own module doc.
//!
//! - `mouse_capture`: `HP2MouseCapturePolicy.h`'s pure decision table
//!   (`OnLoss`/`OnRestore`). FFI-wired: both are thin forwards to
//!   `hp2_rs_mouse_capture_on_loss`/`on_restore`; the C++ decision
//!   logic that used to implement them is deleted. Called from
//!   `USDLViewport::UpdateInput`'s `SDL_WINDOWEVENT` handler
//!   (`SDLViewport.cpp:434/451`, unchanged) and
//!   `Tests/InputContractTests.cpp`'s `TestMouseCapturePolicy`
//!   (`mouse_capture_policy` ctest, unchanged) -- both now run
//!   through Rust.
//! - `input_event`: `HP2InputEvents.h`'s raw-SDL -> normalized-event
//!   mapping functions. FFI-wired: `HP2InputEvents.h`'s five functions
//!   (`HP2MapKeysym`, `HP2MakeKeyEvent`, `HP2MakeMouseButtonEvent`,
//!   `HP2MakeWheelEvents`, `HP2MakeTextEvent`) are thin forwards to
//!   `hp2_rs_input_*`; the C++ switch statement and UTF-8 decoder that
//!   used to implement them are deleted. `USDLViewport::UpdateInput`
//!   (`SDLViewport.cpp:318-480`, five call sites:
//!   `:84-85/345/352/370/378`) and `Tests/InputContractTests.cpp`'s
//!   `TestInputEventMapping` (`input_event_mapping` ctest) are both
//!   unchanged -- they call the same names, which now resolve to Rust.
//! - `clipboard`: `USDLClient::GetClipboardText`/`SetClipboardText`.
//!   FFI-wired; the real C++ caller already calls through it. Ctest:
//!   `clipboard_round_trip` (`Tests/PlatformClipboardTests.cpp`).
//! - `mouse_state`: `USDLViewport::GetViewportButtonFlags`/
//!   `GetMouseState`/`UpdateMouseGrabState` (`SDLViewport.cpp:544-579`).
//!   FFI-wired: all three bodies now call `hp2_rs_platform_*`,
//!   passing `SdlWindow` through as an opaque pointer (the real C++
//!   still owns it; this module never creates/destroys/stores it).
//!   The C++ bodies (the SDL calls and the grab-transition guard)
//!   are deleted; the caller only stores the returned
//!   `MouseIsGrabbed`/`bWindowsMouseAvailable` back into its own
//!   UObject fields, which Rust cannot touch directly. No existing
//!   C++ oracle exercised these (UObject member functions, not
//!   constructible by `Tests/InputContractTests.cpp`'s headless
//!   harness), so `Tests/PlatformMouseStateTests.cpp`
//!   (`mouse_state_round_trip` ctest) is new coverage, not a ported
//!   test.
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
//! scope), joystick open/close
//! (`USDLClient::OpenJoystick`/`CloseJoystick`), and macOS
//! `TextToSpeech` (`fork`/`execl("/usr/bin/say")`, `:770-813`).

pub mod clipboard;
pub mod input_event;
pub mod mouse_capture;
pub mod mouse_state;
