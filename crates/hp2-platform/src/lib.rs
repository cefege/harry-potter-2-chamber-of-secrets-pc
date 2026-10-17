//! Rust port of `HarryPotter2/Unreal/SDLDrv/` -- the SDL2-backed
//! implementation of `UClient`/`UViewport`, registered as
//! `SDLDrv.SDLClient`/`SDLDrv.SDLViewport`.
//!
//! **Scope of this crate today: seven FFI-wired cutovers, not the
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
//! - `joystick_input`: the `SDL_JOYAXISMOTION`/`SDL_JOYBUTTONDOWN`/
//!   `SDL_JOYBUTTONUP`/`SDL_JOYHATMOTION` cases inline in
//!   `UpdateInput` (`SDLViewport.cpp:373-415`). Unlike every other
//!   bullet in this list, there was no existing named C++ function
//!   to forward from -- this logic lived directly in the switch
//!   statement, not behind a header -- so this is new structure
//!   extracted from inline code, not a header-forward of an existing
//!   abstraction. FFI-wired: the three cases now call
//!   `hp2_rs_input_map_joystick_axis`/`map_joystick_button`/
//!   `fold_joystick_hat`, each immediately followed by
//!   `CauseInputEvent` as before. `GJoyAxisKeys` stays in C++ (still
//!   used by `ReleaseJoystickInput`, not converted);
//!   `GHatButtonBase` became dead once this landed and was deleted.
//!   No existing oracle (this logic never had one); covered by
//!   `joystick_input.rs`'s own Rust unit tests plus FFI-boundary
//!   checks added to `Tests/PlatformJoystickTests.cpp`
//!   (`joystick_round_trip` ctest, shared with the `joystick` module
//!   above).
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
//!   covers the real branches: button clamp to 16, axis clamp to
//!   `MaxJoystickAxes` (8), the >12-buttons-and-a-hat fold threshold
//!   (including the exact boundary, 12 vs 14 buttons), `IgnoreHat`
//!   suppression, and out-of-range device index failing cleanly.
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
//! boundary is designed: the rest of window creation/cursor
//! (`OpenWindow`'s `SDL_CreateWindow`/GL-attribute half, no small
//! self-contained piece of it ported yet; `SetModeCursor`, whose only
//! real logic -- clamping `SelectedCursor` against the `SystemCursors`
//! array -- is a C++-owned array index, leaving nothing worth porting
//! on its own), `ReleaseJoystickInput` (still inline C++, now the
//! only remaining consumer of `GJoyAxisKeys`, called in turn by
//! `ReleaseAllInput`),
//! and ungrabbed `SDL_MOUSEMOTION` (dispatches straight to
//! `Engine->MousePosition` with no `HP2Make*Event` call, so it was
//! never part of `input_event`'s scope).
//!
//! `USDLViewport::TextToSpeech`/`UpdateSpeech` (`fork`/
//! `execl("/usr/bin/say")`, `:752-795`) are **not** on this list:
//! confirmed by a repo-wide search, nothing calls `TextToSpeech` at
//! all -- not from any native code, not from UnrealScript, not even
//! declared as a base `UViewport` virtual. `TextToSpeechFile`
//! (`SDLDrv.h:47`) is declared and config-registered but never read.
//! Dead code (AGENTS.md: gets a short comment, not porting effort);
//! left as-is in C++, not a pending Rust slice.

pub mod clipboard;
pub mod input_event;
pub mod joystick;
pub mod joystick_input;
pub mod mouse_capture;
pub mod mouse_state;
pub mod window_chrome;
