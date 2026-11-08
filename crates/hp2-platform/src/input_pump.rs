//! Rust side of `USDLViewport::UpdateInput` (`SDLDrv/Src/SDLViewport.cpp`,
//! 06bfdd3:351-497), `ReleaseAllInput` (06bfdd3:602-624; the cursor here
//! replaces the old `ReleaseJoystickInput` helper), and the repaint-victim
//! predicate `repaint_replaces_best` used by `USDLClient::Tick`
//! (`SDLClient.cpp`, 06bfdd3:185-211).
//!
//! What is here: the SDL event-type dispatch for every event the pump
//! handles, the normalized key/button/wheel/text/joystick mapping, the
//! window loss/restore/resize decisions, the relative-mouse deltas, the
//! ReleaseAllInput iteration order, the stalest-realtime-viewport pick, and
//! (via [`poll_and_flatten`]) the `SDL_PollEvent` call and the flattening of
//! each real `SDL_Event` into [`SdlEvent`] -- this crate's own hand-declared
//! mirror of the pinned SDL2 headers' event union, not a second behavior
//! policy. `viewport_policy::update_input`/`phase` supply
//! `SDL_GetRelativeMouseState` (`mouse_state::get_relative_mouse_state`) and
//! the live window pointer this module's `SDL_GetWindowFlags` call (inside
//! the `WINDOWEVENT` case, via `window_lifecycle::get_window_flags`) needs.
//!
//! What stays in C++ and why: the engine-global services (`UEngine::Key`,
//! `UEngine::MousePosition`, `UEngine::InputEvent`, `UInput::KeyDown`,
//! `UpdateMouseGrabState`, `ResizeViewport`) are reached through the C++
//! executor, which runs the returned [`Action`]s in order. Rust never holds
//! state across an engine call, so an engine exception unwinds only through
//! C++ frames.
//!
//! SDL constants below, and [`poll_and_flatten`]'s event-struct mirrors, are
//! hardcoded from the pinned SDL2 headers (`out/macos-arm64/_deps/
//! hp2_sdl2-src/include/SDL_events.h`, `SDL_video.h`, `SDL_keyboard.h`).
//! `ffi::SdlEventRaw` and its variant structs are cross-checked against the
//! real SDL2 headers by `hp2-ffi`'s `input.rs` (size/offset consts exported
//! through `hp2_rs.h`) and `SDLDrv/Src/SDLViewport.cpp`'s `static_assert`s
//! against `sizeof`/`offsetof` on the genuine `SDL_Event` struct family.
//!
//! Deterministic verification (`HP2_DETERMINISTIC_VERIFICATION=1`, cached by
//! [`deterministic_verification`]) keeps this same policy but drops physical
//! keyboard/text/mouse/joystick events before they can mutate gameplay,
//! skips the relative-mouse sample, and suppresses focus-driven release or
//! restore side effects. Window resize/lifecycle events still run. Controlled
//! replay or Engine input injection never passes through this module, so it
//! stays intact.

use crate::input_event::{self, HP2InputEvent, ik, ist};
use crate::joystick_input::{self, JOY_AXIS_KEYS, MAX_JOYSTICK_AXES};
use crate::mouse_capture::{self, Action as CaptureAction, State, WindowFlags};
use crate::mouse_state;
use std::sync::LazyLock;

/// Process-start environment switch for deterministic verification. Shared
/// with the audio slice so one variable describes the whole run.
pub const DETERMINISTIC_VERIFICATION_ENV: &str = "HP2_DETERMINISTIC_VERIFICATION";

/// Whether this process runs deterministic verification. The switch is the
/// process-start environment variable [`DETERMINISTIC_VERIFICATION_ENV`] set
/// to `1`; absence or any other value leaves production behavior untouched.
/// The answer is cached for the life of the process, so the per-frame pump
/// never reads the environment.
pub fn deterministic_verification() -> bool {
    static FLAG: LazyLock<bool> = LazyLock::new(|| {
        std::env::var_os(DETERMINISTIC_VERIFICATION_ENV)
            .is_some_and(|value| value.to_str() == Some("1"))
    });
    *FLAG
}


/// Process-global gameplay gate: set when the game enters its main loop
/// (after init and any `-LOAD` save travel), read by the input recorder and
/// replayer so load-phase frames can never collide with recorded gameplay
/// timestamps.
pub mod gameplay_gate {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ACTIVE: AtomicBool = AtomicBool::new(false);
    pub fn set_active() {
        ACTIVE.store(true, Ordering::Release);
    }
    pub fn active() -> bool {
        ACTIVE.load(Ordering::Acquire)
    }
}
/// Input recording/replay: `HP2_INPUT_RECORD=<path>` appends one line per
/// polled SDL event (and per-frame relative mouse deltas), tagged with the
/// input-pump call index; `HP2_INPUT_REPLAY=<path>` disables physical polling
/// and feeds the recorded events back through the same translation path at
/// the same call indices, so one recorded play session reproduces
/// indefinitely against any build (pair with `--fixed-dt` for determinism).
mod input_trace {
    use super::SdlEvent;
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::LazyLock;
    use std::sync::Mutex;

    use std::sync::atomic::AtomicU64;
    const TEXT_SIZE: usize = 32;

    enum Mode {
        Off,
        Record { file: Mutex<std::fs::File> },
        Replay { entries: Vec<TraceEntry>, cursor: std::sync::atomic::AtomicUsize },
    }

    struct TraceEntry {
        time: f32,
        relative: Option<(i32, i32)>,
        event: Option<SdlEvent>,
    }

    static MODE: LazyLock<Mode> = LazyLock::new(|| {
        if let Some(path) = std::env::var_os("HP2_INPUT_REPLAY") {
            let data = std::fs::read_to_string(&path).unwrap_or_else(|error| {
                eprintln!("HP2_INPUT_REPLAY: cannot read {path:?}: {error}");
                String::new()
            });
            let mut entries = Vec::new();
            for line in data.lines() {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() < 3 {
                    continue;
                }
                let Ok(time) = fields[0].parse::<f32>() else { continue };
                match fields[1] {
                    "R" if fields.len() >= 4 => {
                        if let (Ok(dx), Ok(dy)) = (fields[2].parse::<i32>(), fields[3].parse::<i32>()) {
                            entries.push(TraceEntry { time, relative: Some((dx, dy)), event: None });
                        }
                    }
                    "E" if fields.len() >= 18 => {
                        let mut text = [0u8; TEXT_SIZE];
                        for (index, slot) in fields[4].split(',').enumerate() {
                            if index >= TEXT_SIZE { break; }
                            if let Ok(byte) = u8::from_str_radix(slot, 16) {
                                text[index] = byte;
                            }
                        }
                        // Layout: time E kind key_sym text motion_x motion_y
                        // motion_state button wheel_y joy_axis joy_axis_value
                        // joy_button joy_hat window_event window_data1
                        // window_data2 window_flags -- 13 numbers after the text.
                        let numbers: Vec<i64> = fields[5..]
                            .iter()
                            .map(|field| field.parse::<i64>().unwrap_or(0))
                            .collect();
                        if numbers.len() < 13 { continue; }
                        let event = SdlEvent {
                            kind: fields[2].parse::<u32>().unwrap_or(0),
                            key_sym: fields[3].parse::<i32>().unwrap_or(0),
                            text,
                            motion_x: numbers[0] as i32,
                            motion_y: numbers[1] as i32,
                            motion_state: numbers[2] as u32,
                            button: numbers[3] as u8,
                            wheel_y: numbers[4] as i32,
                            joy_axis: numbers[5] as u8,
                            joy_axis_value: numbers[6] as i32,
                            joy_button: numbers[7] as u8,
                            joy_hat: numbers[8] as u8,
                            window_event: numbers[9] as u8,
                            window_data1: numbers[10] as i32,
                            window_data2: numbers[11] as i32,
                            window_flags: numbers[12] as u32,
                        };
                        entries.push(TraceEntry { time, relative: None, event: Some(event) });
                    }
                    _ => {}
                }
            }
            Mode::Replay { entries, cursor: std::sync::atomic::AtomicUsize::new(0) }
        } else if let Some(path) = std::env::var_os("HP2_INPUT_RECORD") {
            let file = std::fs::OpenOptions::new().create(true).append(true).open(&path)
                .unwrap_or_else(|error| {
                    eprintln!("HP2_INPUT_RECORD: cannot open {path:?}: {error}");
                    std::fs::File::create("/dev/null").expect("null sink")
                });
            Mode::Record { file: Mutex::new(file) }
        } else {
            Mode::Off
        }
    });

    fn mode() -> &'static Mode {
        &MODE
    }

    pub fn recording() -> bool {
        matches!(mode(), Mode::Record { .. })
    }

    pub fn replaying() -> bool {
        matches!(mode(), Mode::Replay { .. })
    }
    /// Gameplay-relative sample of the level clock: the first pump call after
    /// the main loop starts defines zero for both record and replay, so
    /// load-phase timing differences cancel out. `None` before gameplay.
    pub(crate) fn gameplay_time(raw: f32) -> Option<f32> {
        if !super::gameplay_gate::active() {
            return None;
        }
        static BASE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
        let mut base = BASE.load(std::sync::atomic::Ordering::Relaxed);
        if base == u32::MAX {
            base = raw.to_bits();
            BASE.store(base, std::sync::atomic::Ordering::Relaxed);
        }
        Some(raw - f32::from_bits(base))
    }
    fn write_line(line: String) {
        if let Mode::Record { file } = mode() {
            if let Ok(mut file) = file.lock() {
                let _ = std::io::Write::write_all(&mut *file, line.as_bytes());
            }
        }
    }

    pub fn record_relative(time: f32, dx: i32, dy: i32) {
        write_line(format!("{time:.4} R {dx} {dy}\n"));
    }

    pub fn record_event(time: f32, event: &SdlEvent) {
        let text: Vec<String> = event.text.iter().map(|byte| format!("{byte:02x}")).collect();
        write_line(format!(
            "{time:.4} E {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\n",
            event.kind, event.key_sym, text.join(","),
            event.motion_x, event.motion_y, event.motion_state, event.button,
            event.wheel_y, event.joy_axis, event.joy_axis_value, event.joy_button,
            event.joy_hat, event.window_event, event.window_data1,
            event.window_data2, event.window_flags,
        ));
    }

    /// One replayed pump call: delivers every pending entry recorded at or
    /// before `now` (level `TimeSeconds`), preserving record order.
    pub fn replay_frame(now: f32) -> (Option<(i32, i32)>, Vec<SdlEvent>) {
        match mode() {
            Mode::Replay { entries, cursor } => {
                let mut relative = None;
                let mut events = Vec::new();
                loop {
                    let index = cursor.load(std::sync::atomic::Ordering::Relaxed);
                    let Some(entry) = entries.get(index) else { break };
                    if entry.time > now {
                        break;
                    }
                    cursor.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if let Some((dx, dy)) = entry.relative {
                        relative = Some((dx, dy));
                    }
                    if let Some(event) = entry.event {
                        events.push(event);
                    }
                }
                (relative, events)
            }
            _ => (None, Vec::new()),
        }
    }
}

/// Replay supplies its own relative samples and must not pump physical input
/// as a side effect of a capture transition.
#[cfg(feature = "sdl")]
pub(crate) fn input_replaying() -> bool {
    input_trace::replaying()
}

/// Announces the isolation boundary exactly once per process. The harness
/// requires this marker so a binary that ignores the environment switch
/// cannot be mistaken for one that honored it.
#[cfg(feature = "sdl")]
fn announce_input_isolation() {
    static ANNOUNCED: std::sync::Once = std::sync::Once::new();
    ANNOUNCED.call_once(|| eprintln!("<HP2_VERIFY> input=isolated"));
}

/// `SDL_EventType` values used by the pump.
pub mod sdl_event {
    pub const WINDOWEVENT: u32 = 0x200;
    pub const KEYDOWN: u32 = 0x300;
    pub const KEYUP: u32 = 0x301;
    pub const TEXTEDITING: u32 = 0x302;
    pub const TEXTINPUT: u32 = 0x303;
    pub const MOUSEMOTION: u32 = 0x400;
    pub const MOUSEBUTTONDOWN: u32 = 0x401;
    pub const MOUSEBUTTONUP: u32 = 0x402;
    pub const MOUSEWHEEL: u32 = 0x403;
    pub const JOYAXISMOTION: u32 = 0x600;
    pub const JOYHATMOTION: u32 = 0x602;
    pub const JOYBUTTONDOWN: u32 = 0x603;
    pub const JOYBUTTONUP: u32 = 0x604;
    pub const QUIT: u32 = 0x100;
}

/// `SDL_WindowEventID` values used by the pump (`SDL_video.h`, enum counts
/// from `SDL_WINDOWEVENT_NONE = 0`).
pub mod sdl_window_event {
    pub const SHOWN: u8 = 1;
    pub const HIDDEN: u8 = 2;
    pub const RESIZED: u8 = 5;
    pub const MINIMIZED: u8 = 7;
    pub const RESTORED: u8 = 9;
    pub const FOCUS_GAINED: u8 = 12;
    pub const FOCUS_LOST: u8 = 13;
    pub const CLOSE: u8 = 14;
}

/// `SDL_WINDOW_*` flag bits (`SDL_video.h`).
pub mod sdl_window_flags {
    pub const HIDDEN: u32 = 0x0000_0008;
    pub const MINIMIZED: u32 = 0x0000_0040;
    pub const INPUT_FOCUS: u32 = 0x0000_0200;
}

/// `SDL_TEXTINPUTEVENT_TEXT_SIZE` (`SDL_events.h`).
pub const TEXT_SIZE: usize = 32;

/// Upper bound on actions one event can produce. The widest case is a
/// window loss (`UpdateMouseGrab`, `SetLostGrab`, `ReleaseAll`) or a wheel
/// notch with a joystick hat fold; nothing emits more than three.
pub const MAX_ACTIONS: usize = 4;

/// One polled `SDL_Event`, flattened by [`poll_and_flatten`]. Only the
/// fields the event's own `type` uses are meaningful; the rest are zero.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SdlEvent {
    pub kind: u32,
    /// `keysym.sym` for `KEYDOWN`/`KEYUP`.
    pub key_sym: i32,
    /// `text.text` for `TEXTINPUT`, NUL-padded.
    pub text: [u8; TEXT_SIZE],
    /// `motion.x`/`motion.y`/`motion.state` for `MOUSEMOTION`.
    pub motion_x: i32,
    pub motion_y: i32,
    pub motion_state: u32,
    /// `button.button` for `MOUSEBUTTONDOWN`/`UP`.
    pub button: u8,
    /// `wheel.y` for `MOUSEWHEEL`.
    pub wheel_y: i32,
    /// `jaxis.axis`/`jaxis.value` for `JOYAXISMOTION`.
    pub joy_axis: u8,
    pub joy_axis_value: i32,
    /// `jbutton.button` for `JOYBUTTONDOWN`/`UP`.
    pub joy_button: u8,
    /// `jhat.value` for `JOYHATMOTION`.
    pub joy_hat: u8,
    /// `window.event`, `window.data1`/`data2`, and `SDL_GetWindowFlags` for
    /// `WINDOWEVENT`. `window_flags` is zero when the viewport has no window.
    pub window_event: u8,
    pub window_data1: i32,
    pub window_data2: i32,
    pub window_flags: u32,
}

#[cfg(feature = "sdl")]
pub mod ffi {
    /// `SDL_CommonEvent` (`SDL_events.h:184-188`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlCommonEvent {
        pub kind: u32,
        pub timestamp: u32,
    }
    /// `SDL_Keysym` (`SDL_keyboard.h:48-54`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlKeysym {
        pub scancode: i32,
        pub sym: i32,
        pub modifiers: u16,
        pub unused: u32,
    }
    /// `SDL_KeyboardEvent` (`SDL_events.h:224-234`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlKeyboardEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub state: u8,
        pub repeat: u8,
        pub padding2: u8,
        pub padding3: u8,
        pub keysym: SdlKeysym,
    }
    /// `SDL_TextInputEvent` (`SDL_events.h:288-294`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlTextInputEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub text: [u8; super::TEXT_SIZE],
    }
    /// `SDL_TextEditingEvent` (`SDL_events.h`: preedit composition event).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlTextEditingEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub text: [u8; super::TEXT_SIZE],
        pub start: i32,
        pub length: i32,
    }
    /// `SDL_MouseMotionEvent` (`SDL_events.h:299-310`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlMouseMotionEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub which: u32,
        pub state: u32,
        pub x: i32,
        pub y: i32,
        pub xrel: i32,
        pub yrel: i32,
    }
    /// `SDL_MouseButtonEvent` (`SDL_events.h:315-327`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlMouseButtonEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub which: u32,
        pub button: u8,
        pub state: u8,
        pub clicks: u8,
        pub padding1: u8,
        pub x: i32,
        pub y: i32,
    }
    /// `SDL_MouseWheelEvent` (`SDL_events.h:332-345`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlMouseWheelEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub which: u32,
        pub x: i32,
        pub y: i32,
        pub direction: u32,
        pub precise_x: f32,
        pub precise_y: f32,
        pub mouse_x: i32,
        pub mouse_y: i32,
    }
    /// `SDL_JoyAxisEvent` (`SDL_events.h:350-361`); `which` is
    /// `SDL_JoystickID` (`Sint32`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlJoyAxisEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub which: i32,
        pub axis: u8,
        pub padding1: u8,
        pub padding2: u8,
        pub padding3: u8,
        pub value: i16,
        pub padding4: u16,
    }
    /// `SDL_JoyHatEvent` (`SDL_events.h:382-397`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlJoyHatEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub which: i32,
        pub hat: u8,
        pub value: u8,
        pub padding1: u8,
        pub padding2: u8,
    }
    /// `SDL_JoyButtonEvent` (`SDL_events.h:402-411`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlJoyButtonEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub which: i32,
        pub button: u8,
        pub state: u8,
        pub padding1: u8,
        pub padding2: u8,
    }
    /// `SDL_WindowEvent` (`SDL_events.h:208-219`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlWindowEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub window_id: u32,
        pub event: u8,
        pub padding1: u8,
        pub padding2: u8,
        pub padding3: u8,
        pub data1: i32,
        pub data2: i32,
    }
    /// `SDL_Event` (`SDL_events.h:655-703`): a 56-byte union on every
    /// 64-bit target this builds for (`sizeof(void*) <= 8`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union SdlEventRaw {
        pub kind: u32,
        pub common: SdlCommonEvent,
        pub key: SdlKeyboardEvent,
        pub text: SdlTextInputEvent,
        pub motion: SdlMouseMotionEvent,
        pub button: SdlMouseButtonEvent,
        pub wheel: SdlMouseWheelEvent,
        pub jaxis: SdlJoyAxisEvent,
        pub jbutton: SdlJoyButtonEvent,
        pub jhat: SdlJoyHatEvent,
        pub window: SdlWindowEvent,
        pub padding: [u8; 56],
        // SDL's omitted user/drop events contain pointers and align the union.
        pub pointer_alignment: *mut std::ffi::c_void,
    }

    unsafe extern "C" {
        pub fn SDL_PollEvent(event: *mut SdlEventRaw) -> i32;
    }
}

/// Replaces `Hp2PlatformViewportLeaf_PollEvent`/`FlattenEvent` and the
/// file's own `FlattenSdlEvent` helper (`SDLViewport.cpp:82-116`,
/// `:322-323`). Polls one SDL event and flattens it exactly as the
/// original; returns `None` once the queue is empty (`SDL_PollEvent`
/// returned `0`). The `WINDOWEVENT` case alone additionally reads
/// `window`'s SDL flags, matching the original's `if (V.SdlWindow)` guard.
///
/// # Safety
/// `window`, if non-null, must be a valid `SDL_Window*` for the duration
/// of this call.
#[cfg(feature = "sdl")]
pub unsafe fn poll_and_flatten(window: *mut std::ffi::c_void) -> Option<SdlEvent> {
    let mut raw = ffi::SdlEventRaw { padding: [0u8; 56] };
    // SAFETY: `raw` is a valid, zero-initialized out-param for the
    // duration of the call; SDL's event subsystem is initialized by
    // construction (C++ owns that init and only calls into this crate
    // afterward).
    if unsafe { ffi::SDL_PollEvent(&mut raw) } == 0 {
        return None;
    }
    // SAFETY: `kind` is the union's first field, valid for every variant.
    let kind = unsafe { raw.kind };
    let mut out = SdlEvent {
        kind,
        ..Default::default()
    };
    // SAFETY: `SDL_PollEvent` filled `raw` as the variant selected by
    // `kind`; each arm below reads only that variant.
    unsafe {
        match kind {
            sdl_event::KEYDOWN | sdl_event::KEYUP => out.key_sym = raw.key.keysym.sym,
            sdl_event::TEXTINPUT => out.text = raw.text.text,
            sdl_event::MOUSEMOTION => {
                out.motion_x = raw.motion.x;
                out.motion_y = raw.motion.y;
                out.motion_state = raw.motion.state;
            }
            sdl_event::MOUSEBUTTONDOWN | sdl_event::MOUSEBUTTONUP => out.button = raw.button.button,
            sdl_event::MOUSEWHEEL => out.wheel_y = raw.wheel.y,
            sdl_event::JOYAXISMOTION => {
                out.joy_axis = raw.jaxis.axis;
                out.joy_axis_value = raw.jaxis.value as i32;
            }
            sdl_event::JOYBUTTONDOWN | sdl_event::JOYBUTTONUP => {
                out.joy_button = raw.jbutton.button
            }
            sdl_event::JOYHATMOTION => out.joy_hat = raw.jhat.value,
            sdl_event::WINDOWEVENT => {
                out.window_event = raw.window.event;
                out.window_data1 = raw.window.data1;
                out.window_data2 = raw.window.data2;
                if !window.is_null() {
                    // SAFETY: non-null checked; validity is this
                    // function's own contract (see above).
                    out.window_flags = crate::window_lifecycle::get_window_flags(window.cast());
                }
            }
            _ => {}
        }
    }
    Some(out)
}

/// Per-event snapshot of the `USDLViewport`/`USDLClient` fields the pump reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context {
    /// `MouseIsGrabbed`.
    pub mouse_is_grabbed: bool,
    /// `LostGrab`.
    pub lost_grab: bool,
    /// `UClient::CaptureMouse` (`C->CaptureMouse`).
    pub capture_mouse: bool,
    /// `SdlWindow != NULL`.
    pub has_window: bool,
    /// `C->Engine != NULL`.
    pub has_engine: bool,
    /// `Actor != NULL`.
    pub has_actor: bool,
    /// `C->JoyHatFoldsToButtons`.
    pub joy_hat_folds_to_buttons: bool,
    /// `MouseScaleX`/`MouseScaleY`.
    pub mouse_scale_x: f32,
    pub mouse_scale_y: f32,
    /// `SizeX`/`SizeY`.
    pub size_x: i32,
    pub size_y: i32,
    /// `C->JoystickDeadZone`, `C->ScaleJBX`, `C->ScaleJBY`.
    pub joystick_dead_zone: f32,
    pub scale_jbx: f32,
    pub scale_jby: f32,
    /// `LastJoyHat`.
    pub last_joy_hat: i32,
}

/// One side effect for the C++ executor, in original call order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// `CauseInputEvent(key, action, delta)`.
    Cause { key: i32, action: i32, delta: f32 },
    /// `ClientEngine->Key(this, key)`.
    EngineKey { key: i32 },
    /// `C->Engine->MousePosition(this, buttons, x, y)`.
    EngineMousePosition { buttons: u32, x: f32, y: f32 },
    /// `UpdateMouseGrabState(capture)`.
    UpdateMouseGrab { capture: bool },
    /// `LostGrab = value`.
    SetLostGrab(bool),
    /// `LastJoyHat = key`.
    SetLastJoyHat(i32),
    /// `ReleaseAllInput()`; the C++ executor runs [`release_all_next`].
    ReleaseAll,
    /// `ResizeViewport(BLIT_NoWindowChange, width, height, INDEX_NONE)`.
    ResizeViewport { width: i32, height: i32 },
}

/// Fixed-capacity, order-preserving action list (no allocation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActionList {
    items: [Option<Action>; MAX_ACTIONS],
    len: usize,
}

impl ActionList {
    pub fn new() -> Self {
        Self {
            items: [None; MAX_ACTIONS],
            len: 0,
        }
    }

    fn push(&mut self, action: Action) {
        assert!(
            self.len < MAX_ACTIONS,
            "input pump produced more than MAX_ACTIONS actions"
        );
        self.items[self.len] = Some(action);
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = Action> + '_ {
        self.items[..self.len].iter().flatten().copied()
    }
}

impl Default for ActionList {
    fn default() -> Self {
        Self::new()
    }
}

fn push_normalized(out: &mut ActionList, event: HP2InputEvent) {
    if event.key != ik::NONE {
        out.push(Action::Cause {
            key: event.key,
            action: event.action,
            delta: 0.0,
        });
    }
}

/// Translates one polled event under the process verification switch.
pub fn translate_event(ctx: &Context, ev: &SdlEvent) -> ActionList {
    translate_event_with_verification(ctx, ev, deterministic_verification())
}

/// Translates one polled event. Mirrors the `switch (Event.type)` in
/// `UpdateInput`; every arm keeps the original's guards and call order.
///
/// With `verification` set, the physical producers are dropped before they
/// can reach a gameplay mutation: SDL keyboard, text, mouse motion/button/
/// wheel, and joystick events produce no [`Action`]s. Window events still go
/// through [`translate_window`], which keeps resize handling and suppresses
/// the loss/restore grab-and-release side effects. Controlled replay or
/// Engine input injection never enters this translator, so it is unaffected.
pub fn translate_event_with_verification(
    ctx: &Context,
    ev: &SdlEvent,
    verification: bool,
) -> ActionList {
    let mut out = ActionList::new();
    match ev.kind {
        sdl_event::KEYDOWN | sdl_event::KEYUP if !verification => {
            let normalized = input_event::make_key_event(ev.key_sym, ev.kind == sdl_event::KEYDOWN);
            push_normalized(&mut out, normalized);
        }
        sdl_event::TEXTINPUT if !verification => {
            let len = ev.text.iter().position(|&b| b == 0).unwrap_or(TEXT_SIZE);
            let normalized = input_event::make_text_event(Some(&ev.text[..len]));
            if normalized.key != ik::NONE && ctx.has_engine && ctx.has_actor {
                out.push(Action::EngineKey {
                    key: normalized.key,
                });
            }
        }
        sdl_event::MOUSEMOTION if !verification => {
            if !ctx.mouse_is_grabbed && ctx.has_engine {
                out.push(Action::EngineMousePosition {
                    buttons: mouse_state::buttons_from_sdl_state(ev.motion_state),
                    x: ev.motion_x as f32 * ctx.mouse_scale_x,
                    y: ev.motion_y as f32 * ctx.mouse_scale_y,
                });
            }
        }
        sdl_event::MOUSEBUTTONDOWN | sdl_event::MOUSEBUTTONUP if !verification => {
            let normalized = input_event::make_mouse_button_event(
                ev.button,
                ev.kind == sdl_event::MOUSEBUTTONDOWN,
            );
            push_normalized(&mut out, normalized);
        }
        sdl_event::MOUSEWHEEL if !verification => {
            let mut wheel = [input_event::make_none_event(); 3];
            let count = input_event::make_wheel_events(ev.wheel_y, &mut wheel);
            for event in wheel.iter().take(count as usize) {
                out.push(Action::Cause {
                    key: event.key,
                    action: event.action,
                    delta: event.delta,
                });
            }
        }
        sdl_event::JOYAXISMOTION if !verification => {
            if let Some((key, value)) = joystick_input::map_joystick_axis(
                ev.joy_axis as i32,
                ev.joy_axis_value,
                ctx.joystick_dead_zone,
                ctx.scale_jbx,
                ctx.scale_jby,
            ) {
                out.push(Action::Cause {
                    key,
                    action: ist::AXIS,
                    delta: value,
                });
            }
        }
        sdl_event::JOYBUTTONDOWN | sdl_event::JOYBUTTONUP if !verification => {
            if let Some((key, action)) = joystick_input::map_joystick_button(
                ev.joy_button as i32,
                ev.kind == sdl_event::JOYBUTTONDOWN,
            ) {
                out.push(Action::Cause {
                    key,
                    action,
                    delta: 0.0,
                });
            }
        }
        sdl_event::JOYHATMOTION if !verification => {
            if ctx.joy_hat_folds_to_buttons {
                let (new_last, events, count) =
                    joystick_input::fold_joystick_hat(ev.joy_hat, ctx.last_joy_hat);
                for &(key, action) in events.iter().take(count) {
                    out.push(Action::Cause {
                        key,
                        action,
                        delta: 0.0,
                    });
                }
                out.push(Action::SetLastJoyHat(new_last));
            }
        }
        sdl_event::WINDOWEVENT => translate_window(ctx, ev, verification, &mut out),
        _ => {}
    }
    out
}

/// Notify the owner of a real focus-gained event before legacy grab actions.
/// Replayed and deterministic-verification input must not notify an owner.
#[cfg(feature = "overlay-mouse")]
fn observe_focus_gained<E>(ev: &SdlEvent, verification: bool, mut notify: impl FnMut() -> Result<(), E>) -> Result<(), E> {
    if !verification && ev.kind == sdl_event::WINDOWEVENT && ev.window_event == sdl_window_event::FOCUS_GAINED {
        notify()?;
    }
    Ok(())
}

fn translate_window(ctx: &Context, ev: &SdlEvent, verification: bool, out: &mut ActionList) {
    use sdl_window_event as we;

    let kind = ev.window_event;
    let loss = kind == we::FOCUS_LOST || kind == we::MINIMIZED || kind == we::HIDDEN;
    let restore = kind == we::FOCUS_GAINED || kind == we::RESTORED || kind == we::SHOWN;
    let state = State {
        currently_grabbed: ctx.mouse_is_grabbed,
        loss_pending: ctx.lost_grab,
        capture_mouse_config: ctx.capture_mouse,
    };

    if loss {
        // Deterministic verification never turns a focus loss into input
        // state: releasing the grab is meaningless (no grab was taken) and
        // `ReleaseAll` would also drop keys injected by a controlled replay.
        if verification {
            return;
        }
        if matches!(
            mouse_capture::on_loss(&state).action,
            CaptureAction::Release
        ) {
            out.push(Action::UpdateMouseGrab { capture: false });
            out.push(Action::SetLostGrab(true));
        }
        // Held input dies on every loss event, grabbed or not.
        out.push(Action::ReleaseAll);
    } else if restore && ctx.has_window {
        // Focus regain must not re-enter OS mouse capture during
        // verification (it would trap the user's cursor and re-couple the
        // session to physical focus changes).
        if verification {
            return;
        }
        let flags = WindowFlags {
            has_input_focus: ev.window_flags & sdl_window_flags::INPUT_FOCUS != 0,
            minimized_or_hidden: ev.window_flags
                & (sdl_window_flags::MINIMIZED | sdl_window_flags::HIDDEN)
                != 0,
        };
        if matches!(
            mouse_capture::on_restore(&state, &flags).action,
            CaptureAction::Restore
        ) {
            out.push(Action::UpdateMouseGrab {
                capture: ctx.capture_mouse,
            });
            out.push(Action::SetLostGrab(false));
        }
    } else if kind == we::RESIZED {
        // Wayland compositors can resize the xdg-toplevel out from under us
        // (tiling layout, compositor-driven fullscreen) without ever routing
        // through ToggleFullscreen/EndFullscreen. Left unhandled, SizeX/SizeY,
        // MouseScaleX/Y, and the render device's resolution go stale while
        // SDL's own window size (and therefore SDL_MOUSEMOTION coordinates)
        // track the new size, so the menu cursor drifts away from the pointer
        // by the gap between the old and new size.
        let (width, height) = (ev.window_data1, ev.window_data2);
        if width > 0 && height > 0 && (width != ctx.size_x || height != ctx.size_y) {
            out.push(Action::ResizeViewport { width, height });
        }
    }
}

/// Relative-mouse deltas sampled once per pump while grabbed (the C++ shell
/// gates the `SDL_GetRelativeMouseState` call on `MouseIsGrabbed && SdlWindow`).
pub fn relative_mouse(ctx: &Context, delta_x: i32, delta_y: i32) -> ActionList {
    let mut out = ActionList::new();
    if delta_x != 0 {
        out.push(Action::Cause {
            key: ik::MOUSE_X,
            action: ist::AXIS,
            delta: delta_x as f32 * ctx.mouse_scale_x,
        });
    }
    if delta_y != 0 {
        out.push(Action::Cause {
            key: ik::MOUSE_Y,
            action: ist::AXIS,
            delta: -(delta_y as f32) * ctx.mouse_scale_y,
        });
    }
    out
}

/// Mouse axes neutralized last by `ReleaseAllInput`, in original order.
const RELEASE_MOUSE_AXES: [i32; 3] = [ik::MOUSE_X, ik::MOUSE_Y, ik::MOUSE_W];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleasePhase {
    Keys,
    JoyAxes,
    JoyHat,
    Mouse,
    Done,
}

/// Iteration state of `ReleaseAllInput`, advanced by [`release_all_next`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseAllCursor {
    pub phase: ReleasePhase,
    pub index: i32,
    pub awaiting_key: bool,
}

/// One step of `ReleaseAllInput`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReleaseStep {
    /// The executor must answer `Input->KeyDown(key)` and pass it back
    /// as `held` on the next [`release_all_next`] call.
    QueryKeyDown(i32),
    /// `CauseInputEvent(key, action, delta)`.
    Cause { key: i32, action: i32, delta: f32 },
    /// `LastJoyHat = key`.
    SetLastJoyHat(i32),
}

/// Starts `ReleaseAllInput`. `has_input` is `Input != NULL`; when false the
/// per-key loop is skipped, exactly like the original `if( Input )` guard.
pub fn release_all_begin(has_input: bool) -> ReleaseAllCursor {
    ReleaseAllCursor {
        phase: if has_input {
            ReleasePhase::Keys
        } else {
            ReleasePhase::JoyAxes
        },
        index: 0,
        awaiting_key: false,
    }
}

/// Advances `ReleaseAllInput`. `held` answers the most recent
/// [`ReleaseStep::QueryKeyDown`]; it is ignored otherwise. Returns `None`
/// once the sequence is finished.
pub fn release_all_next(cursor: &mut ReleaseAllCursor, held: bool) -> Option<ReleaseStep> {
    loop {
        match cursor.phase {
            ReleasePhase::Keys => {
                if cursor.awaiting_key {
                    cursor.awaiting_key = false;
                    let key = cursor.index;
                    cursor.index += 1;
                    if held {
                        return Some(ReleaseStep::Cause {
                            key,
                            action: ist::RELEASE,
                            delta: 0.0,
                        });
                    }
                } else if cursor.index < ik::MAX {
                    cursor.awaiting_key = true;
                    return Some(ReleaseStep::QueryKeyDown(cursor.index));
                } else {
                    cursor.phase = ReleasePhase::JoyAxes;
                    cursor.index = 0;
                }
            }
            ReleasePhase::JoyAxes => {
                if cursor.index < MAX_JOYSTICK_AXES {
                    let key = JOY_AXIS_KEYS[cursor.index as usize];
                    cursor.index += 1;
                    return Some(ReleaseStep::Cause {
                        key,
                        action: ist::AXIS,
                        delta: 0.0,
                    });
                }
                cursor.phase = ReleasePhase::JoyHat;
            }
            ReleasePhase::JoyHat => {
                cursor.phase = ReleasePhase::Mouse;
                cursor.index = 0;
                return Some(ReleaseStep::SetLastJoyHat(ik::NONE));
            }
            ReleasePhase::Mouse => {
                if (cursor.index as usize) < RELEASE_MOUSE_AXES.len() {
                    let key = RELEASE_MOUSE_AXES[cursor.index as usize];
                    cursor.index += 1;
                    return Some(ReleaseStep::Cause {
                        key,
                        action: ist::AXIS,
                        delta: 0.0,
                    });
                }
                cursor.phase = ReleasePhase::Done;
            }
            ReleasePhase::Done => return None,
        }
    }
}

/// Typed operations used to execute the shared SDL input policy.
pub trait UpdateInputBackend {
    /// Failure type supplied by the owning Engine/Core adapter.
    type Error;

    /// Rebinds the viewport's owning client at the original method boundary.
    fn bind_client(&mut self) -> Result<(), Self::Error>;
    /// Reads `MouseIsGrabbed` at the source's branch boundary.
    fn mouse_is_grabbed(&mut self) -> Result<bool, Self::Error>;
    /// Reads the live input fields used by the pure event translator.
    fn context(&mut self) -> Result<Context, Self::Error>;
    /// Reports whether `UViewport::Input` is live.
    fn has_input(&mut self) -> Result<bool, Self::Error>;
    /// Returns the live level's `TimeSeconds` for input replay alignment.
    fn game_time(&mut self) -> Result<f32, Self::Error>;
    /// Returns this viewport's live SDL window, or null.
    fn window(&mut self) -> Result<*mut std::ffi::c_void, Self::Error>;
    fn key_down(&mut self, key: i32) -> Result<bool, Self::Error>;
    /// Dispatches one `CauseInputEvent`.
    fn cause_input(&mut self, key: i32, action: i32, delta: f32) -> Result<(), Self::Error>;
    /// Executes non-input side effects in the returned action sequence.
    fn execute_action(&mut self, action: Action) -> Result<(), Self::Error>;
    /// Stores `LastJoyHat`.
    fn set_last_joy_hat(&mut self, key: i32) -> Result<(), Self::Error>;
    /// Called before reset or focus-loss releases, while all held keys remain available.
    fn before_release_all(&mut self) -> Result<(), Self::Error> { Ok(()) }
    /// Observation only; called for real SDL focus-gained events before legacy
    /// capture actions, regardless of whether the viewport had a lost grab.
    #[cfg(feature = "overlay-mouse")]
    fn observe_focus_gained(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

/// Executes SDL polling, reset release, relative mouse, and translated events
/// through a typed host without duplicating the pump's mapping algorithms.
///
/// In deterministic verification the relative-mouse hardware sampling and
/// every physical gameplay event are skipped; window events still flow so
/// resizing and window lifecycle keep working, and the single isolation
/// marker is emitted on the first pump.
///
/// # Errors
/// Returns the first host operation failure, stopping later actions in order.
///
/// # Safety
/// `window` results must point to a live SDL window until each poll completes.
#[cfg(feature = "sdl")]
pub unsafe fn update_input_with<B: UpdateInputBackend>(
    backend: &mut B,
    reset: bool,
) -> Result<(), B::Error> {
    let verification = deterministic_verification();
    if verification {
        announce_input_isolation();
    }
    backend.bind_client()?;
    if reset {
        release_all_with(backend)?;
    }
    if input_trace::replaying() {
        if let Some(now) = input_trace::gameplay_time(backend.game_time()?) {
            let (relative, events) = input_trace::replay_frame(now);
            if let Some((dx, dy)) = relative {
                let context = backend.context()?;
                let actions = relative_mouse(&context, dx, dy);
                execute_actions(backend, &actions)?;
            }
            for event in &events {
                let context = backend.context()?;
                let actions = translate_event_with_verification(&context, event, verification);
                execute_actions(backend, &actions)?;
            }
        }
        return Ok(());
    }
    if !verification && backend.mouse_is_grabbed()? && !backend.window()?.is_null() {
        let (dx, dy) = crate::mouse_state::get_relative_mouse_state();
        if input_trace::recording() {
            if let Some(now) = input_trace::gameplay_time(backend.game_time()?) {
                input_trace::record_relative(now, dx, dy);
            }
        }
        let context = backend.context()?;
        let actions = relative_mouse(&context, dx, dy);
        execute_actions(backend, &actions)?;
    }
    loop {
        let window = backend.window()?;
        // SAFETY: the backend contract guarantees the returned non-null SDL
        // window remains live for this synchronous poll.
        let Some(event) = (unsafe { poll_and_flatten(window) }) else {
            break;
        };
        if input_trace::recording() && event.kind != 0x200 && event.kind != 0x100 {
            if let Some(now) = input_trace::gameplay_time(backend.game_time()?) {
                input_trace::record_event(now, &event);
            }
        }
        let context = backend.context()?;
        #[cfg(feature = "overlay-mouse")]
        observe_focus_gained(&event, verification, || backend.observe_focus_gained())?;
        let actions = translate_event_with_verification(&context, &event, verification);
        execute_actions(backend, &actions)?;
    }
    Ok(())
}

#[cfg(feature = "sdl")]
pub(crate) fn release_all_with<B: UpdateInputBackend>(backend: &mut B) -> Result<(), B::Error> {
    backend.before_release_all()?;
    let mut cursor = release_all_begin(backend.has_input()?);
    let mut held = false;
    while let Some(step) = release_all_next(&mut cursor, held) {
        held = false;
        match step {
            ReleaseStep::QueryKeyDown(key) => held = backend.key_down(key)?,
            ReleaseStep::Cause { key, action, delta } => backend.cause_input(key, action, delta)?,
            ReleaseStep::SetLastJoyHat(key) => backend.set_last_joy_hat(key)?,
        }
    }
    Ok(())
}

#[cfg(feature = "sdl")]
fn execute_actions<B: UpdateInputBackend>(
    backend: &mut B,
    actions: &ActionList,
) -> Result<(), B::Error> {
    for action in actions.iter() {
        match action {
            Action::Cause { key, action, delta } => backend.cause_input(key, action, delta)?,
            Action::SetLastJoyHat(key) => backend.set_last_joy_hat(key)?,
            Action::ReleaseAll => release_all_with(backend)?,
            action => backend.execute_action(action)?,
        }
    }
    Ok(())
}

/// One non-null entry of `USDLClient::Viewports` as seen by `Tick`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RepaintCandidate {
    /// `Viewport->IsRealtime()`.
    pub realtime: bool,
    /// `Viewport->SizeX`/`SizeY`.
    pub size_x: i32,
    pub size_y: i32,
    /// `(victim->LastUpdateTime - Viewport->LastUpdateTime)` as computed by
    /// `FTime::operator-`, which returns the exact sign of the 64-bit fixed-point
    /// difference. Positive means the victim is strictly more recent than this
    /// viewport. Passing the sign, rather than two rounded floats, keeps Tick's
    /// comparison identical to the original `FTime` `>` comparison.
    pub best_minus_candidate: f32,
}

/// Whether `USDLClient::Tick` replaces the current repaint victim with
/// `candidate`. An ineligible candidate never replaces. An eligible one replaces
/// when there is no victim yet, or when the victim's time is strictly greater
/// (`best_minus_candidate > 0`); equal times keep the earlier viewport.
pub fn repaint_replaces_best(has_best: bool, candidate: RepaintCandidate) -> bool {
    if !(candidate.realtime && candidate.size_x != 0 && candidate.size_y != 0) {
        return false;
    }
    !has_best || candidate.best_minus_candidate > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            mouse_is_grabbed: false,
            lost_grab: false,
            capture_mouse: false,
            has_window: true,
            has_engine: true,
            has_actor: true,
            joy_hat_folds_to_buttons: false,
            mouse_scale_x: 1.0,
            mouse_scale_y: 1.0,
            size_x: 800,
            size_y: 600,
            joystick_dead_zone: 0.0,
            scale_jbx: 1.0,
            scale_jby: 1.0,
            last_joy_hat: ik::NONE,
        }
    }

    fn event(kind: u32) -> SdlEvent {
        SdlEvent {
            kind,
            key_sym: 0,
            text: [0; TEXT_SIZE],
            motion_x: 0,
            motion_y: 0,
            motion_state: 0,
            button: 0,
            wheel_y: 0,
            joy_axis: 0,
            joy_axis_value: 0,
            joy_button: 0,
            joy_hat: 0,
            window_event: 0,
            window_data1: 0,
            window_data2: 0,
            window_flags: 0,
        }
    }

    fn window(kind: u8, flags: u32) -> SdlEvent {
        SdlEvent {
            window_event: kind,
            window_flags: flags,
            ..event(sdl_event::WINDOWEVENT)
        }
    }

    fn collect(list: &ActionList) -> Vec<Action> {
        list.iter().collect()
    }

    #[test]
    fn sdl_constants_match_pinned_header_values() {
        assert_eq!(sdl_event::WINDOWEVENT, 0x200);
        assert_eq!(sdl_event::KEYDOWN, 0x300);
        assert_eq!(sdl_event::KEYUP, 0x301);
        assert_eq!(sdl_event::TEXTINPUT, 0x303);
        assert_eq!(sdl_event::MOUSEMOTION, 0x400);
        assert_eq!(sdl_event::MOUSEBUTTONDOWN, 0x401);
        assert_eq!(sdl_event::MOUSEBUTTONUP, 0x402);
        assert_eq!(sdl_event::MOUSEWHEEL, 0x403);
        assert_eq!(sdl_event::JOYAXISMOTION, 0x600);
        assert_eq!(sdl_event::JOYHATMOTION, 0x602);
        assert_eq!(sdl_event::JOYBUTTONDOWN, 0x603);
        assert_eq!(sdl_event::JOYBUTTONUP, 0x604);
        assert_eq!(sdl_window_event::SHOWN, 1);
        assert_eq!(sdl_window_event::HIDDEN, 2);
        assert_eq!(sdl_window_event::RESIZED, 5);
        assert_eq!(sdl_window_event::MINIMIZED, 7);
        assert_eq!(sdl_window_event::RESTORED, 9);
        assert_eq!(sdl_window_event::FOCUS_GAINED, 12);
        assert_eq!(sdl_window_event::FOCUS_LOST, 13);
        assert_eq!(sdl_window_flags::HIDDEN, 0x8);
        assert_eq!(sdl_window_flags::MINIMIZED, 0x40);
        assert_eq!(sdl_window_flags::INPUT_FOCUS, 0x200);
    }

    #[test]
    fn key_down_emits_cause_for_mapped_key() {
        let mut ev = event(sdl_event::KEYDOWN);
        ev.key_sym = b'a' as i32;
        let out = collect(&translate_event(&ctx(), &ev));
        assert_eq!(out.len(), 1);
        assert!(matches!(
            out[0],
            Action::Cause { key, action, delta } if key == ik::A && action == ist::PRESS && delta == 0.0
        ));
    }

    #[test]
    fn key_with_no_mapping_emits_nothing() {
        let mut ev = event(sdl_event::KEYDOWN);
        ev.key_sym = 0x1F600;
        assert!(translate_event(&ctx(), &ev).is_empty());
    }

    #[test]
    fn text_input_stops_at_nul_and_requires_engine_and_actor() {
        let mut ev = event(sdl_event::TEXTINPUT);
        ev.text[0] = b'a';
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![Action::EngineKey { key: b'a' as i32 }]
        );
        let mut no_actor = ctx();
        no_actor.has_actor = false;
        assert!(translate_event(&no_actor, &ev).is_empty());
    }

    #[test]
    fn mouse_motion_matches_oracle_when_ungrabbed() {
        // Oracle: `harness.cpp` scenario `motion_ungrabbed` (motion x=10, y=20,
        // state=4 right button, scale 0.5/0.25). Trace: `MousePosition buttons=2
        // x=5.0 y=5.0`. The grabbed scenario emits nothing.
        let mut ev = event(sdl_event::MOUSEMOTION);
        ev.motion_x = 10;
        ev.motion_y = 20;
        ev.motion_state = 4;
        let mut c = ctx();
        c.mouse_scale_x = 0.5;
        c.mouse_scale_y = 0.25;
        assert_eq!(
            collect(&translate_event(&c, &ev)),
            vec![Action::EngineMousePosition {
                buttons: 2,
                x: 5.0,
                y: 5.0,
            }]
        );
        c.mouse_is_grabbed = true;
        assert!(translate_event(&c, &ev).is_empty());
    }

    #[test]
    fn wheel_matches_oracle_one_pair_per_call() {
        // Oracle: `harness.cpp` scenario `wheel` (wheel_y=2, -1, 0). wheel_y=2
        // traces `231 axis 2.0`, `236 press`, `236 release`; wheel_y=-1 traces
        // `231 axis -1.0`, `237 press`, `237 release`; wheel_y=0 emits nothing.
        let mut ev = event(sdl_event::MOUSEWHEEL);
        ev.wheel_y = 2;
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_W,
                    action: ist::AXIS,
                    delta: 2.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_UP,
                    action: ist::PRESS,
                    delta: 0.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_UP,
                    action: ist::RELEASE,
                    delta: 0.0,
                },
            ]
        );
        ev.wheel_y = -1;
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_W,
                    action: ist::AXIS,
                    delta: -1.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_DOWN,
                    action: ist::PRESS,
                    delta: 0.0,
                },
                Action::Cause {
                    key: ik::MOUSE_WHEEL_DOWN,
                    action: ist::RELEASE,
                    delta: 0.0,
                },
            ]
        );
        ev.wheel_y = 0;
        assert!(translate_event(&ctx(), &ev).is_empty());
    }

    #[test]
    fn hat_fold_respects_flag_and_writes_last_hat() {
        let mut ev = event(sdl_event::JOYHATMOTION);
        ev.joy_hat = 1;
        let mut c = ctx();
        assert!(translate_event(&c, &ev).is_empty());
        c.joy_hat_folds_to_buttons = true;
        let (new_last, events, count) = joystick_input::fold_joystick_hat(1, c.last_joy_hat);
        let mut expected: Vec<Action> = events[..count]
            .iter()
            .map(|&(key, action)| Action::Cause {
                key,
                action,
                delta: 0.0,
            })
            .collect();
        expected.push(Action::SetLastJoyHat(new_last));
        assert_eq!(collect(&translate_event(&c, &ev)), expected);
    }

    #[test]
    fn keys_match_oracle() {
        // Oracle: `harness.cpp` scenario `keys`. SDLK_a press/release is
        // key 65 (action 1/3); SDLK_UNKNOWN emits nothing; SDLK_LSHIFT
        // (0x400000E1) is key 160 press/release.
        let key = |kind, sym| {
            let mut ev = event(kind);
            ev.key_sym = sym;
            ev
        };
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYDOWN, b'a' as i32)
            )),
            vec![Action::Cause {
                key: 65,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYUP, b'a' as i32)
            )),
            vec![Action::Cause {
                key: 65,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&ctx(), &key(sdl_event::KEYDOWN, 0)).is_empty());
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYDOWN, 0x4000_00E1)
            )),
            vec![Action::Cause {
                key: 160,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &key(sdl_event::KEYUP, 0x4000_00E1)
            )),
            vec![Action::Cause {
                key: 160,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
    }

    #[test]
    fn mouse_buttons_match_oracle() {
        // Oracle: `harness.cpp` scenario `buttons`. SDL button 1 down is key 1
        // (press), 2 up is key 4 (release), 3 down is key 2 (press), 4 is unmapped.
        let button = |kind, b| {
            let mut ev = event(kind);
            ev.button = b;
            ev
        };
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::MOUSEBUTTONDOWN, 1)
            )),
            vec![Action::Cause {
                key: 1,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::MOUSEBUTTONUP, 2)
            )),
            vec![Action::Cause {
                key: 4,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::MOUSEBUTTONDOWN, 3)
            )),
            vec![Action::Cause {
                key: 2,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&ctx(), &button(sdl_event::MOUSEBUTTONDOWN, 4)).is_empty());
    }

    #[test]
    fn joystick_axis_matches_oracle_scales() {
        // Oracle: `harness.cpp` scenario `joy_axis` (deadzone 0.1, ScaleJBX=2,
        // ScaleJBY=0.5). Axis 0 at 16384 is 1.0; axis 1 at -32768 is -0.5;
        // axis 2 at 1000 is inside the deadzone and emits 0; axis 8 is dropped.
        let mut c = ctx();
        c.joystick_dead_zone = 0.1;
        c.scale_jbx = 2.0;
        c.scale_jby = 0.5;
        let axis = |index: u8, value: i32| {
            let mut ev = event(sdl_event::JOYAXISMOTION);
            ev.joy_axis = index;
            ev.joy_axis_value = value;
            ev
        };
        assert_eq!(
            collect(&translate_event(&c, &axis(0, 16384))),
            vec![Action::Cause {
                key: 224,
                action: ist::AXIS,
                delta: 1.0,
            }]
        );
        assert_eq!(
            collect(&translate_event(&c, &axis(1, -32768))),
            vec![Action::Cause {
                key: 225,
                action: ist::AXIS,
                delta: -0.5,
            }]
        );
        assert_eq!(
            collect(&translate_event(&c, &axis(2, 1000))),
            vec![Action::Cause {
                key: 226,
                action: ist::AXIS,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&c, &axis(8, 16384)).is_empty());
    }

    #[test]
    fn joystick_button_matches_oracle() {
        // Oracle: `harness.cpp` scenario `joy_button`. Button 3 down is key 203
        // (press); button 16 is dropped; button 0 up is key 200 (release).
        let button = |kind, b| {
            let mut ev = event(kind);
            ev.joy_button = b;
            ev
        };
        assert_eq!(
            collect(&translate_event(
                &ctx(),
                &button(sdl_event::JOYBUTTONDOWN, 3)
            )),
            vec![Action::Cause {
                key: 203,
                action: ist::PRESS,
                delta: 0.0,
            }]
        );
        assert!(translate_event(&ctx(), &button(sdl_event::JOYBUTTONDOWN, 16)).is_empty());
        assert_eq!(
            collect(&translate_event(&ctx(), &button(sdl_event::JOYBUTTONUP, 0))),
            vec![Action::Cause {
                key: 200,
                action: ist::RELEASE,
                delta: 0.0,
            }]
        );
    }

    #[test]
    fn hat_fold_matches_oracle() {
        // Oracle: `harness.cpp` scenario `hat_fold` (UP|RIGHT = 3, then centered).
        // UP|RIGHT presses key 212 only; centered releases 212.
        let mut c = ctx();
        c.joy_hat_folds_to_buttons = true;
        let mut hat = event(sdl_event::JOYHATMOTION);
        hat.joy_hat = 3;
        let pressed = collect(&translate_event(&c, &hat));
        assert_eq!(
            pressed[0],
            Action::Cause {
                key: 212,
                action: ist::PRESS,
                delta: 0.0,
            }
        );
        assert!(matches!(pressed.last(), Some(Action::SetLastJoyHat(_))));
        c.last_joy_hat = 212;
        hat.joy_hat = 0;
        let released = collect(&translate_event(&c, &hat));
        assert_eq!(
            released[0],
            Action::Cause {
                key: 212,
                action: ist::RELEASE,
                delta: 0.0,
            }
        );
    }

    #[test]
    fn loss_while_grabbed_releases_then_releases_all() {
        let mut c = ctx();
        c.mouse_is_grabbed = true;
        let out = collect(&translate_event(
            &c,
            &window(sdl_window_event::FOCUS_LOST, 0),
        ));
        assert_eq!(
            out,
            vec![
                Action::UpdateMouseGrab { capture: false },
                Action::SetLostGrab(true),
                Action::ReleaseAll,
            ]
        );
    }

    #[test]
    fn loss_while_ungrabbed_only_releases_all() {
        let out = collect(&translate_event(
            &ctx(),
            &window(sdl_window_event::MINIMIZED, 0),
        ));
        assert_eq!(out, vec![Action::ReleaseAll]);
    }

    #[test]
    fn restore_completes_pending_grab_when_focused() {
        let mut c = ctx();
        c.lost_grab = true;
        c.capture_mouse = true;
        let out = collect(&translate_event(
            &c,
            &window(
                sdl_window_event::FOCUS_GAINED,
                sdl_window_flags::INPUT_FOCUS,
            ),
        ));
        assert_eq!(
            out,
            vec![
                Action::UpdateMouseGrab { capture: true },
                Action::SetLostGrab(false),
            ]
        );
    }

    #[cfg(feature = "overlay-mouse")]
    #[test]
    fn focus_gained_observed_with_and_without_a_lost_grab() {
        let ev = window(sdl_window_event::FOCUS_GAINED, sdl_window_flags::INPUT_FOCUS);
        for lost in [false, true] {
            let mut c = ctx(); c.lost_grab = lost; c.capture_mouse = true;
            let mut observations = 0;
            observe_focus_gained(&ev, false, || { observations += 1; Ok::<(), ()>(()) }).unwrap();
            assert_eq!(observations, 1);
            let actions = collect(&translate_event_with_verification(&c, &ev, false));
            if lost {
                assert_eq!(actions, vec![Action::UpdateMouseGrab { capture: true }, Action::SetLostGrab(false)]);
            } else { assert!(actions.is_empty()); }
        }
    }

    #[cfg(feature = "overlay-mouse")]
    #[test]
    fn focus_observation_excludes_other_and_verification_events_and_propagates_errors() {
        let gained = window(sdl_window_event::FOCUS_GAINED, sdl_window_flags::INPUT_FOCUS);
        let lost = window(sdl_window_event::FOCUS_LOST, 0);
        let mut observations = 0;
        observe_focus_gained(&gained, true, || { observations += 1; Ok::<(), &str>(()) }).unwrap();
        observe_focus_gained(&lost, false, || { observations += 1; Ok::<(), &str>(()) }).unwrap();
        assert_eq!(observations, 0);
        assert_eq!(observe_focus_gained(&gained, false, || Err("observer")), Err("observer"));
    }

    #[test]
    fn restore_without_window_is_ignored() {
        let mut c = ctx();
        c.has_window = false;
        c.lost_grab = true;
        assert!(
            translate_event(
                &c,
                &window(sdl_window_event::SHOWN, sdl_window_flags::INPUT_FOCUS),
            )
            .is_empty()
        );
    }

    #[test]
    fn resize_applies_only_positive_changed_sizes() {
        let mut ev = window(sdl_window_event::RESIZED, 0);
        ev.window_data1 = 1024;
        ev.window_data2 = 768;
        assert_eq!(
            collect(&translate_event(&ctx(), &ev)),
            vec![Action::ResizeViewport {
                width: 1024,
                height: 768,
            }]
        );
        ev.window_data1 = 800;
        ev.window_data2 = 600;
        assert!(translate_event(&ctx(), &ev).is_empty());
        ev.window_data1 = 0;
        ev.window_data2 = 600;
        assert!(translate_event(&ctx(), &ev).is_empty());
    }

    #[test]
    fn resize_matches_oracle_640x480_to_800x600() {
        // Oracle: `harness.cpp` scenario `resize` (viewport 640x480). A real size
        // emits `ResizeViewport` (`ResizeViewport vp=1 flags=32 x=800 y=600`);
        // a zero size emits nothing.
        let mut c = ctx();
        c.size_x = 640;
        c.size_y = 480;
        let mut ev = window(sdl_window_event::RESIZED, 0);
        ev.window_data1 = 800;
        ev.window_data2 = 600;
        assert_eq!(
            collect(&translate_event(&c, &ev)),
            vec![Action::ResizeViewport {
                width: 800,
                height: 600,
            }]
        );
        ev.window_data1 = 0;
        ev.window_data2 = 0;
        assert!(translate_event(&c, &ev).is_empty());
    }

    #[test]
    fn relative_mouse_matches_oracle_scales() {
        // Oracle: `harness.cpp` scenario `rel_mouse_grabbed` (scale 0.5/0.25,
        // dx=3, dy=-2) and `rel_mouse_window_scale` (scale 1.6 from 640/400
        // and 480/300; `MouseScaleX=3fcccccd`).
        let mut c = ctx();
        c.mouse_scale_x = 0.5;
        c.mouse_scale_y = 0.25;
        assert_eq!(
            collect(&relative_mouse(&c, 3, -2)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_X,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x3fc0_0000), // 1.5
                },
                Action::Cause {
                    key: ik::MOUSE_Y,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x3f00_0000), // 0.5
                },
            ]
        );
        c.mouse_scale_x = f32::from_bits(0x3fcc_cccd);
        c.mouse_scale_y = f32::from_bits(0x3fcc_cccd);
        assert_eq!(
            collect(&relative_mouse(&c, 3, -2)),
            vec![
                Action::Cause {
                    key: ik::MOUSE_X,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x4099_999a), // 4.8
                },
                Action::Cause {
                    key: ik::MOUSE_Y,
                    action: ist::AXIS,
                    delta: f32::from_bits(0x404c_cccd), // 3.2
                },
            ]
        );
        assert!(collect(&relative_mouse(&c, 0, 0)).is_empty());
    }

    /// Drives [`release_all_next`] like the C++ executor: answers each
    /// `QueryKeyDown` from `held`, and returns every step in order.
    fn drive_release_all(has_input: bool, held: &[i32]) -> Vec<ReleaseStep> {
        let mut cursor = release_all_begin(has_input);
        let mut steps = Vec::new();
        let mut answer = false;
        while let Some(step) = release_all_next(&mut cursor, answer) {
            answer = false;
            if let ReleaseStep::QueryKeyDown(key) = step {
                answer = held.contains(&key);
            }
            steps.push(step);
        }
        steps
    }

    /// Steps after the per-key loop: 8 joystick axes, the hat, then the
    /// three mouse axes. Matches the oracle's `release_all_empty` trace.
    fn joy_and_mouse_release_tail() -> Vec<ReleaseStep> {
        let mut tail = Vec::new();
        for key in [224, 225, 226, 227, 232, 233, 234, 235] {
            tail.push(ReleaseStep::Cause {
                key,
                action: ist::AXIS,
                delta: 0.0,
            });
        }
        tail.push(ReleaseStep::SetLastJoyHat(ik::NONE));
        for key in [228, 229, 231] {
            tail.push(ReleaseStep::Cause {
                key,
                action: ist::AXIS,
                delta: 0.0,
            });
        }
        tail
    }

    #[test]
    fn release_all_matches_oracle_with_held_keys() {
        // Oracle: `harness.cpp` scenario `reset_release` (IK_A=65, IK_Shift=16,
        // IK_Joy1=200 held; LastJoyHat=IK_Joy5; UpdateInput(1)). Trace order:
        // 16, 65, 200 RELEASE; then the joystick and mouse tail above.
        let steps = drive_release_all(true, &[16, 65, 200]);
        let queries = steps
            .iter()
            .filter(|s| matches!(s, ReleaseStep::QueryKeyDown(_)))
            .count();
        assert_eq!(queries as i32, ik::MAX);
        let actions: Vec<ReleaseStep> = steps
            .into_iter()
            .filter(|s| !matches!(s, ReleaseStep::QueryKeyDown(_)))
            .collect();
        let mut expected = vec![
            ReleaseStep::Cause {
                key: 16,
                action: ist::RELEASE,
                delta: 0.0,
            },
            ReleaseStep::Cause {
                key: 65,
                action: ist::RELEASE,
                delta: 0.0,
            },
            ReleaseStep::Cause {
                key: 200,
                action: ist::RELEASE,
                delta: 0.0,
            },
        ];
        expected.extend(joy_and_mouse_release_tail());
        assert_eq!(actions, expected);
    }

    #[test]
    fn release_all_empty_matches_oracle() {
        // Oracle: `harness.cpp` scenario `release_all_empty`. No key is held, so
        // only the joystick and mouse tail is emitted.
        let steps = drive_release_all(true, &[]);
        let actions: Vec<ReleaseStep> = steps
            .into_iter()
            .filter(|s| !matches!(s, ReleaseStep::QueryKeyDown(_)))
            .collect();
        assert_eq!(actions, joy_and_mouse_release_tail());
    }

    #[test]
    fn release_all_without_input_skips_keys() {
        let mut cursor = release_all_begin(false);
        let mut count = 0;
        while let Some(step) = release_all_next(&mut cursor, false) {
            assert!(!matches!(step, ReleaseStep::QueryKeyDown(_)));
            count += 1;
        }
        assert_eq!(
            count,
            MAX_JOYSTICK_AXES as usize + 1 + RELEASE_MOUSE_AXES.len()
        );
    }

    #[test]
    fn repaint_replaces_best_matches_tick_predicate() {
        let cand = |realtime, best_minus_candidate| RepaintCandidate {
            realtime,
            size_x: 4,
            size_y: 4,
            best_minus_candidate,
        };
        assert!(!repaint_replaces_best(false, cand(false, 0.0)));
        assert!(repaint_replaces_best(false, cand(true, 0.0)));
        assert!(!repaint_replaces_best(true, cand(true, 0.0)));
        assert!(repaint_replaces_best(true, cand(true, 3.0)));
        assert!(!repaint_replaces_best(true, cand(true, -3.0)));
        let zero_size = RepaintCandidate {
            realtime: true,
            size_x: 0,
            size_y: 4,
            best_minus_candidate: 1.0,
        };
        assert!(!repaint_replaces_best(false, zero_size));
    }

    /// Index of the viewport `Tick` repaints, given candidates in array order.
    /// Mirrors the fold in `USDLClient::Tick` (710119c `SDLClient.cpp`).
    fn tick_victim(candidates: &[RepaintCandidate]) -> Option<usize> {
        let mut best = None;
        for (index, candidate) in candidates.iter().enumerate() {
            if repaint_replaces_best(best.is_some(), *candidate) {
                best = Some(index);
            }
        }
        best
    }

    #[test]
    fn tick_victim_matches_oracle() {
        // Oracle: `/tmp/hp2-m2/input/harness.cpp` scenarios `tick_victim`,
        // `tick_tie`, `tick_subfloat_order`. Build and run:
        //   clang++ -std=c++17 -w -I shim harness.cpp -o harness && ./harness
        // `best_minus_candidate` is the sign of (best - candidate) that the
        // C++ fold computes with `FTime::operator-`.
        let cand = |realtime: bool, size_x: i32, size_y: i32, best_minus_candidate: f32| {
            RepaintCandidate {
                realtime,
                size_x,
                size_y,
                best_minus_candidate,
            }
        };
        // a t=5.0 (realtime, 4x4), b t=2.0, c t=2.0, d t=0.5 (not realtime),
        // e t=0.1 (SizeX=0). Oracle repaints Id 2 (b).
        let victim = [
            cand(true, 4, 4, 0.0),
            cand(true, 4, 4, 3.0),
            cand(true, 4, 4, 0.0),
            cand(false, 4, 4, 1.5),
            cand(true, 0, 4, 1.9),
        ];
        assert_eq!(tick_victim(&victim), Some(1));
        // a t=3.0, b t=3.0: a tie keeps the earlier viewport (Id 1).
        let tie = [cand(true, 4, 4, 0.0), cand(true, 4, 4, 0.0)];
        assert_eq!(tick_victim(&tie), Some(0));
        // a = 1000.0 + 1e-7, b = 1000.0: a - b is positive (bits 0x33d68000),
        // so b is the stale one (Id 2).
        let subfloat = [
            cand(true, 4, 4, 0.0),
            cand(true, 4, 4, f32::from_bits(0x33d6_8000)),
        ];
        assert_eq!(tick_victim(&subfloat), Some(1));
    }

    #[test]
    fn verification_drops_every_physical_gameplay_producer() {
        let mut c = ctx();
        c.joy_hat_folds_to_buttons = true;
        c.last_joy_hat = ik::NONE;

        let mut key_down = event(sdl_event::KEYDOWN);
        key_down.key_sym = b'a' as i32;
        let mut key_up = event(sdl_event::KEYUP);
        key_up.key_sym = b'a' as i32;
        let mut text = event(sdl_event::TEXTINPUT);
        text.text[0] = b'a';
        let mut motion = event(sdl_event::MOUSEMOTION);
        motion.motion_x = 10;
        motion.motion_y = 20;
        motion.motion_state = 4;
        let mut button_down = event(sdl_event::MOUSEBUTTONDOWN);
        button_down.button = 1;
        let mut button_up = event(sdl_event::MOUSEBUTTONUP);
        button_up.button = 1;
        let mut wheel = event(sdl_event::MOUSEWHEEL);
        wheel.wheel_y = 2;
        let mut axis = event(sdl_event::JOYAXISMOTION);
        axis.joy_axis = 0;
        axis.joy_axis_value = 32767;
        let mut joy_button = event(sdl_event::JOYBUTTONDOWN);
        joy_button.joy_button = 0;
        let mut hat = event(sdl_event::JOYHATMOTION);
        hat.joy_hat = 1;

        let physical = [
            key_down, key_up, text, motion, button_down, button_up, wheel, axis, joy_button, hat,
        ];
        for ev in physical {
            assert!(
                !translate_event_with_verification(&c, &ev, false).is_empty(),
                "production translation must emit actions for event kind {:#x}",
                ev.kind
            );
            assert!(
                translate_event_with_verification(&c, &ev, true).is_empty(),
                "verification must drop physical event kind {:#x}",
                ev.kind
            );
        }
    }

    #[test]
    fn verification_keeps_resize_and_suppresses_focus_transitions() {
        // Resize is real window lifecycle, not hardware input: it still runs.
        let mut resize = window(sdl_window_event::RESIZED, 0);
        resize.window_data1 = 1024;
        resize.window_data2 = 768;
        assert_eq!(
            collect(&translate_event_with_verification(&ctx(), &resize, true)),
            vec![Action::ResizeViewport {
                width: 1024,
                height: 768,
            }]
        );

        // Focus loss normally drops the grab and releases every held key,
        // including keys injected by a controlled replay.
        let mut grabbed = ctx();
        grabbed.mouse_is_grabbed = true;
        grabbed.capture_mouse = true;
        let lost = window(sdl_window_event::FOCUS_LOST, 0);
        assert_eq!(
            collect(&translate_event_with_verification(&grabbed, &lost, false)),
            vec![
                Action::UpdateMouseGrab { capture: false },
                Action::SetLostGrab(true),
                Action::ReleaseAll,
            ]
        );
        assert!(translate_event_with_verification(&grabbed, &lost, true).is_empty());
        for kind in [sdl_window_event::MINIMIZED, sdl_window_event::HIDDEN] {
            assert!(
                translate_event_with_verification(&grabbed, &window(kind, 0), true).is_empty()
            );
        }

        // Focus regain normally re-enters OS mouse capture.
        let mut pending = ctx();
        pending.lost_grab = true;
        pending.capture_mouse = true;
        let gained = window(sdl_window_event::FOCUS_GAINED, sdl_window_flags::INPUT_FOCUS);
        assert_eq!(
            collect(&translate_event_with_verification(&pending, &gained, false)),
            vec![
                Action::UpdateMouseGrab { capture: true },
                Action::SetLostGrab(false),
            ]
        );
        assert!(translate_event_with_verification(&pending, &gained, true).is_empty());
    }
}
