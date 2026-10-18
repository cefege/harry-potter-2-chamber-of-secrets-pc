//! Rust port of `HarryPotter2/Unreal/SDLDrv/` -- the SDL2-backed
//! implementation of `UClient`/`UViewport`, registered as
//! `SDLDrv.SDLClient`/`SDLDrv.SDLViewport`.
//!
//! **Scope of this crate today: eight FFI-wired cutovers, not the
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
//! - `input_pump`: `USDLViewport::UpdateInput`'s event-to-action
//!   translation (`SDLViewport.cpp:351-497`), `ReleaseAllInput`'s
//!   iteration order (`:602-624`), and the repaint-victim predicate that
//!   `USDLClient::Tick` folds over (`SDLClient.cpp:185-211`). The C++ shell
//!   flattens each polled `SDL_Event` into an `SdlEvent`, snapshots its UObject
//!   fields into a `Context`, calls `translate_event`, and runs the returned
//!   `Action`s in order (`CauseInputEvent`, `Engine->Key`, `Engine->MousePosition`,
//!   `UpdateMouseGrabState`, `ResizeViewport`, `ReleaseAllInput`, and the
//!   `LastJoyHat`/`LostGrab` writes). Rust never holds state across an engine
//!   call. Pure modules used only by
//!   `input_pump`: `input_event` (the raw-SDL key/button/wheel/text
//!   mapping, previously `HP2InputEvents.h`), `joystick_input` (axis,
//!   button, and hat mapping), and `mouse_capture` (the loss/restore
//!   decision table, previously `HP2MouseCapturePolicy.h`). Ctests:
//!   `input_edge_contracts`, `input_axis_order`, `input_release_all`
//!   (`Tests/InputContractTests.cpp`); the pure mapping and capture
//!   decisions are unit-tested in Rust (`input_pump`, `input_event`,
//!   `mouse_capture`).
//! - `clipboard`: `USDLClient::GetClipboardText`/`SetClipboardText`.
//!   FFI-wired; the real C++ caller already calls through it. Ctest:
//!   `clipboard_round_trip` (`Tests/PlatformClipboardTests.cpp`).
//! - `mouse_state`: `USDLViewport::GetViewportButtonFlags`/
//!   `GetMouseState`/`UpdateMouseGrabState` (`SDLViewport.cpp:544-579`).
//!   FFI-wired: all three bodies call `hp2_rs_platform_*`, passing
//!   `SdlWindow` through as an opaque pointer (the real C++ still owns
//!   it; this module never creates/destroys/stores it). The caller only
//!   stores the returned `MouseIsGrabbed`/`bWindowsMouseAvailable` back
//!   into its own UObject fields. `buttons_from_sdl_state` is also used
//!   by `input_pump`. Ctest: `mouse_state_round_trip`.
//! - `window_chrome`: `USDLViewport::UpdateWindowFrame`/`SetTitleBar`
//!   (`SDLViewport.cpp:233-239/748-755`). FFI-wired: both bodies now
//!   call `hp2_rs_platform_*`, `SdlWindow` passed through as an
//!   opaque pointer (same design as `mouse_state`). The C++ `if
//!   (SdlWindow) SDL_SetWindowBordered`/`SDL_SetWindowTitle` calls are
//!   deleted. No existing oracle (same reason as `mouse_state`); new
//!   coverage in `Tests/PlatformWindowChromeTests.cpp`
//!   (`window_chrome_round_trip` ctest) -- a genuine round trip for
//!   `set_title_bar` (set, then `SDL_GetWindowTitle` to compare), but
//!   only a does-not-crash check for `update_window_frame`: SDL's
//!   "dummy" video driver doesn't update
//!   `SDL_GetWindowFlags`/`SDL_WINDOW_BORDERLESS` in response to
//!   `SDL_SetWindowBordered` (confirmed empirically, not assumed),
//!   so there is nothing stronger to assert under that driver --
//!   the `borderless` inversion itself is pinned by a plain Rust
//!   unit test instead (`sdl_bordered_flag_inverts_borderless`,
//!   no SDL driver needed).
//! - `joystick`: `USDLClient::OpenJoystick`/`CloseJoystick`
//!   (`SDLClient.cpp:234-274`). FFI-wired: both bodies now call
//!   `hp2_rs_platform_*`; the real C++'s own `Joystick`/`JoyButtons`/
//!   `JoyHats`/`JoyAxes`/`JoyHatFoldsToButtons` fields are set from
//!   the FFI call's results (Rust does not own them). Unlike every
//!   other cutover here, the device-present path is genuinely
//!   exercisable headless: `SDL_JoystickAttachVirtual` (SDL2
//!   2.0.14+) creates a software joystick with no real hardware,
//!   confirmed empirically to work under the dummy video driver.
//!   `Tests/PlatformJoystickTests.cpp` (`joystick_round_trip` ctest)
//!   covers the real branches: button clamp to 16, axis clamp to 8,
//!   the >12-buttons-and-a-hat fold threshold (including the exact
//!   boundary, 12 vs 14 buttons), `IgnoreHat`
//!   suppression, and out-of-range device index failing cleanly.
//! - `viewport`: pure decision logic from `USDLViewport`
//!   (`SDLViewport.cpp`): `UpdateMouseScale`'s math, the constructor's
//!   desktop colour-depth choice, `OpenWindow`'s size/flags/fullscreen
//!   choice, `ResizeViewport`'s blit-flag and size transitions,
//!   `IsFullscreen`, `SetModeCursor`'s index clamp, and `Lock`'s gate.
//!   FFI-wired through `crates/hp2-ffi/src/viewport.rs`; the C++ shell
//!   keeps every `SDL_*` call and `UViewport` field write. Reference
//!   values were captured from the verbatim C++ bodies with clang++.
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
//! What remains in C++ and why: the `SDL_*` calls (`SDL_PollEvent`,
//! `SDL_GetRelativeMouseState`, `SDL_GetWindowFlags`, window/cursor
//! calls, joystick open/close) and the flattening of each `SDL_Event`
//! into `SdlEvent`, because they operate on C++-owned SDL objects.
//! `OpenWindow`'s `SDL_CreateWindow`/GL-attribute half and
//! `SetModeCursor`'s `SDL_SetCursor` call also stay (the index clamp is
//! `hp2_rs_viewport_mode_cursor_index`). Every engine call stays in C++
//! -- `CauseInputEvent`'s `Engine->InputEvent`, `Engine->Key`,
//! `Engine->MousePosition`, `Input->KeyDown`, `ResizeViewport`,
//! `UpdateMouseGrabState` -- and the UObject field reads and writes:
//! Rust must never hold a borrow across an engine call that can throw,
//! so Rust returns an action list and C++ executes it in order.
//! `SetMouseCapture` stays in C++ for the same SDL reason.
//!
//! `USDLViewport::TextToSpeech`/`UpdateSpeech` (`fork`/
//! `execl("/usr/bin/say")`, `:752-795`) are **not** on this list:
//! confirmed by a repo-wide search, nothing calls `TextToSpeech` at
//! all -- not from any native code, not from UnrealScript, not even
//! declared as a base `UViewport` virtual. `TextToSpeechFile`
//! (`SDLDrv.h:47`) is declared and config-registered but never read.
//! Confirmed dead code, zero real callers; the user chose "leave it
//! alone" (not delete, not port, not wire up) when asked, so this
//! stays as-is in C++ -- not a pending Rust slice, and not scheduled
//! for deletion either. Revisit only if SDLDrv's remaining C++ gets
//! swept up for an unrelated reason.

pub mod clipboard;
pub mod input_event;
pub mod input_pump;
pub mod joystick;
pub mod joystick_input;
pub mod mouse_capture;
pub mod mouse_state;
pub mod viewport;
pub mod window_chrome;
