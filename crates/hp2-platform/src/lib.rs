//! Rust policy for the SDL2-backed `SDLDrv.SDLClient` and
//! `SDLDrv.SDLViewport`. Native registered UObject shells retain SDK layout,
//! virtual dispatch, typed temporaries, and SDL resource fields.
//!
//! `client_policy` owns client defaults, initialization, configuration,
//! normal/crash shutdown, live viewport ticking/repaint selection, and joystick
//! lifecycle. `viewport_policy` owns viewport construction/destruction, window
//! and cursor lifecycle, render-device selection, lock/unlock, resize/fullscreen,
//! input polling/action execution, command dispatch, and title/mouse policy.
//! Checked native leaves perform genuine SDK operations. Invocation-local native
//! scopes retain title conversion storage, FTime values, and original
//! guard/signal lifetimes through nested Rust calls.
//! Native exceptions are rethrown only after Rust returns; no exclusive native
//! borrow or arena lock spans a callback.
//!
//! - `input_pump` translates each flattened SDL event using the live native
//!   input context. `viewport_policy` executes its actions in order and rereads
//!   mutable engine/viewport state at the original callback boundaries.
//!   `input_event`, `joystick_input`, and `mouse_capture` supply the pure mapping
//!   and loss/restore decisions. Registered contracts: `input_edge_contracts`,
//!   `input_axis_order`, `input_release_all`, and `input_script_smoke`.
//! - `viewport` supplies geometry, color depth, window flags/fullscreen choice,
//!   resize transitions, cursor clamping, and lock eligibility directly to
//!   `viewport_policy`. The superseded decision-only viewport FFI is removed.
//! - `clipboard`, `cursor`, `display_mode`, `mouse_state`, `sdl_runtime`,
//!   `window_chrome`, and `window_lifecycle` implement direct SDL
//!   operations. Their standalone FFI remains exercised by
//!   `clipboard_round_trip`, `mouse_state_round_trip`, and
//!   `window_chrome_round_trip`. Complete viewport SDK qualification
//!   additionally exercises native window behavior; dummy SDL alone does
//!   not prove Cocoa window flags or rendering.
//! - `joystick` implements actual SDL device lifecycle/capability policy for
//!   `client_policy`. `joystick_round_trip` uses genuine SDL virtual devices;
//!   physical joystick hardware is not covered.
//!
//! Complete original/current SDK consumers exercise native callback faults,
//! reentry, live mutations, real Cocoa/wgpu device operations, and crash teardown.
//! Exact commands, raw clock classifications, data-profile limits, and installed
//! game acceptance are recorded in `Docs/BEHAVIOR_MATRIX.md`.
//!
//! `speech` owns the complete macOS speech process lifecycle, including the
//! disabled fast path, nonblocking reaping, fork/exec, and failure semantics.
//! The native viewport embeds Rust-defined inline state and supplies only a
//! checked FString-to-UTF8 leaf. This dormant entry point is retained rather
//! than wired into new callers; native viewport ownership still depends on the
//! separate object-foundation cutover.

pub mod client_policy;
pub mod clipboard;
#[cfg(unix)]
pub mod crash_reporter;
pub mod cursor;
pub mod display_mode;
pub mod game_controller;
pub mod input_event;
pub mod input_options;
pub mod input_pump;
pub mod joystick;
pub mod joystick_input;
pub mod keyboard_focus;
pub mod launcher_boot_video;
pub mod launcher_delay;
pub mod launcher_display_modes;
pub mod launcher_splash;
pub mod live_display_mode;
pub mod message_box;
pub mod mouse_capture;
pub mod mouse_state;
pub mod picker;
pub mod reveal;
pub mod sdl_runtime;
pub mod speech;
pub mod viewport;
pub mod viewport_policy;
pub mod window_chrome;
pub mod window_lifecycle;

/// `HP2_BACKGROUND=1`: run without touching the desktop (debugging, replays, captures).
/// The window is created hidden, the app never activates or takes focus (macOS
/// background app, no Dock icon), and the mouse is never grabbed, hidden or warped.
/// Rendering continues off screen, so `HP2_CAPTURE_FRAMES` still works.
pub fn background_mode() -> bool {
    std::env::var_os("HP2_BACKGROUND").is_some_and(|v| v == "1")
}
