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
//! policy.
//!
//! `hp2-engine`'s `platform_owner::update_input` constructs a
//! `CoreInputBackend` and calls `update_input_with` when `sdl` is enabled.
//! The backend supplies live viewport and input state and executes the
//! resulting actions; the pump samples relative mouse state and polls SDL.
//! `platform_owner::client_tick` also calls `repaint_replaces_best`.
//!
//! SDL constants and the hand-declared event mirrors derive from the pinned
//! SDL2 headers (`out/macos-arm64/_deps/hp2_sdl2-src/include/SDL_events.h`,
//! `SDL_video.h`, `SDL_keyboard.h`). Historical cross-checks referenced
//! `hp2-ffi`'s `input.rs`, `hp2_rs.h`, and C++ `static_assert`s in
//! `SDLDrv/Src/SDLViewport.cpp`. Those describe the former native build,
//! not checks established for the current build graph.
//!
//! Deterministic verification (`HP2_DETERMINISTIC_VERIFICATION=1`, cached by
//! [`deterministic_verification`]) suppresses keyboard, text, mouse, and
//! joystick event actions, skips physical relative-mouse sampling, and
//! suppresses focus-driven release or restore side effects. Window resize
//! events still run. Replayed events also use verification-aware translation;
//! replayed relative samples are passed directly to [`relative_mouse`].

use crate::game_controller;
use crate::input_event::{self, HP2InputEvent, ik, ist};
use crate::input_options;
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
/// Input recording/replay uses gameplay-relative level-clock time.
/// `HP2_INPUT_RECORD=<path>` appends sampled relative mouse deltas and polled
/// events other than window and quit events once gameplay is active.
/// `HP2_INPUT_REPLAY=<path>` disables physical polling and supplies due trace
/// entries to the input pump.
mod input_trace {
    use super::SdlEvent;
    use std::sync::LazyLock;
    use std::sync::Mutex;

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

    /// Consumes entries through `now` in gameplay-relative level-clock time.
    /// Scanning stops at the first entry later than `now`. The result retains
    /// the last due relative sample and all due events in event order. The
    /// pump dispatches that relative sample before the events.
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
    pub use crate::game_controller::event::{
        AXISMOTION as CONTROLLERAXISMOTION, BUTTONDOWN as CONTROLLERBUTTONDOWN,
        BUTTONUP as CONTROLLERBUTTONUP, DEVICEADDED as CONTROLLERDEVICEADDED,
        DEVICEREMOVED as CONTROLLERDEVICEREMOVED,
    };
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
    /// `SDL_ControllerAxisEvent` (`SDL_events.h`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlControllerAxisEvent {
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
    /// `SDL_ControllerButtonEvent` (`SDL_events.h`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlControllerButtonEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub which: i32,
        pub button: u8,
        pub state: u8,
        pub padding1: u8,
        pub padding2: u8,
    }
    /// `SDL_ControllerDeviceEvent` (`SDL_events.h`); `which` is the device
    /// index for additions and the instance id for removals.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct SdlControllerDeviceEvent {
        pub kind: u32,
        pub timestamp: u32,
        pub which: i32,
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
        pub caxis: SdlControllerAxisEvent,
        pub cbutton: SdlControllerButtonEvent,
        pub cdevice: SdlControllerDeviceEvent,
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
            sdl_event::CONTROLLERAXISMOTION => {
                out.joy_axis = raw.caxis.axis;
                out.joy_axis_value = raw.caxis.value as i32;
            }
            sdl_event::CONTROLLERBUTTONDOWN | sdl_event::CONTROLLERBUTTONUP => {
                out.joy_button = raw.cbutton.button
            }
            // Device index (added) / instance id (removed), carried in `motion_x`.
            sdl_event::CONTROLLERDEVICEADDED | sdl_event::CONTROLLERDEVICEREMOVED => {
                out.motion_x = raw.cdevice.which
            }
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

#[derive(Default)]
struct MenuPointer {
    position: Option<(f32, f32)>,
    stick: [f32; 2],
    dpad: [bool; 4],
    held: [Option<i32>; 15],
    mouse_buttons: u32,
    last_step: Option<std::time::Instant>,
}

impl MenuPointer {
    fn dimensions(ctx: &Context) -> (f32, f32) {
        let width = ctx.size_x as f32 / ctx.mouse_scale_x.max(f32::EPSILON);
        let height = ctx.size_y as f32 / ctx.mouse_scale_y.max(f32::EPSILON);
        (width.max(1.0), height.max(1.0))
    }

    fn position_action(&mut self, ctx: &Context) -> Action {
        let (w, h) = Self::dimensions(ctx);
        let (x, y) = self.position.get_or_insert((w * 0.5, h * 0.5));
        *x = x.clamp(0.0, (w - 1.0).max(0.0));
        *y = y.clamp(0.0, (h - 1.0).max(0.0));
        let virtual_left = self.held.iter().any(|key| *key == Some(ik::LEFT_MOUSE));
        Action::EngineMousePosition {
            buttons: self.mouse_buttons | u32::from(virtual_left),
            x: *x * ctx.mouse_scale_x,
            y: *y * ctx.mouse_scale_y,
        }
    }

    fn step(&mut self, ctx: &Context, dt: f32) -> Option<Action> {
        let curve = |v: f32| {
            let magnitude = ((v.abs() - 0.2) / 0.8).clamp(0.0, 1.0);
            v.signum() * magnitude * magnitude
        };
        let dx = curve(self.stick[0]) * 1.2 + (self.dpad[3] as i32 - self.dpad[2] as i32) as f32 * 0.45;
        let dy = curve(self.stick[1]) * 1.2 + (self.dpad[1] as i32 - self.dpad[0] as i32) as f32 * 0.45;
        if dx == 0.0 && dy == 0.0 {
            return None;
        }
        let (w, h) = Self::dimensions(ctx);
        let (x, y) = self.position.get_or_insert((w * 0.5, h * 0.5));
        *x = (*x + dx * w * dt).clamp(0.0, (w - 1.0).max(0.0));
        *y = (*y + dy * w * dt).clamp(0.0, (h - 1.0).max(0.0));
        Some(self.position_action(ctx))
    }

    fn button(&mut self, button: i32, pressed: bool, menu: bool) -> Option<(i32, i32)> {
        let index = usize::try_from(button).ok().filter(|&i| i < self.held.len())?;
        if (game_controller::button::DPAD_UP..=game_controller::button::DPAD_RIGHT).contains(&button) {
            self.dpad[(button - game_controller::button::DPAD_UP) as usize] = pressed;
        }
        let key = if pressed {
            let key = self.held[index].or_else(|| match (menu, button) {
                (true, game_controller::button::A) => Some(ik::LEFT_MOUSE),
                (true, game_controller::button::B) => Some(ik::ESCAPE),
                _ => game_controller::button_key(button),
            })?;
            self.held[index] = Some(key);
            key
        } else {
            self.held[index].take().or_else(|| game_controller::button_key(button))?
        };
        Some((key, if pressed { ist::PRESS } else { ist::RELEASE }))
    }
}

#[cfg(feature = "sdl")]
static MENU_POINTER: LazyLock<std::sync::Mutex<MenuPointer>> =
    LazyLock::new(|| std::sync::Mutex::new(MenuPointer::default()));

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
/// through `translate_window`, which keeps resize handling and suppresses
/// the loss/restore grab-and-release side effects. Controlled replay or
/// Engine input injection never enters this translator, so it is unaffected.
pub fn translate_event_with_verification(
    ctx: &Context,
    ev: &SdlEvent,
    verification: bool,
) -> ActionList {
    translate_event_inner(ctx, ev, verification, None, false)
}

fn translate_event_inner(
    ctx: &Context,
    ev: &SdlEvent,
    verification: bool,
    mut pointer: Option<&mut MenuPointer>,
    menu: bool,
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
            if let Some(pointer) = pointer.as_deref_mut() {
                if !ctx.mouse_is_grabbed {
                    pointer.position = Some((ev.motion_x as f32, ev.motion_y as f32));
                    pointer.mouse_buttons = mouse_state::buttons_from_sdl_state(ev.motion_state);
                }
            }
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
        sdl_event::JOYAXISMOTION if !verification && !game_controller::active() => {
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
        sdl_event::JOYBUTTONDOWN | sdl_event::JOYBUTTONUP if !verification && !game_controller::active() => {
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
        sdl_event::JOYHATMOTION if !verification && !game_controller::active() => {
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
        sdl_event::CONTROLLERAXISMOTION if !verification => {
            if let Some(pointer) = pointer.as_deref_mut() {
                if (ev.joy_axis as i32) <= game_controller::axis::LEFT_Y {
                    pointer.stick[ev.joy_axis as usize] = (ev.joy_axis_value as f32 / 32767.0).clamp(-1.0, 1.0);
                }
            }
            let events = game_controller::translate_axis(
                ev.joy_axis as i32,
                ev.joy_axis_value,
                ctx.joystick_dead_zone,
                ctx.scale_jbx,
                ctx.scale_jby,
                input_options::gamepad_look_sensitivity(),
            );
            for (key, action, delta) in events.iter() {
                out.push(Action::Cause { key, action, delta });
            }
        }
        sdl_event::CONTROLLERBUTTONDOWN | sdl_event::CONTROLLERBUTTONUP if !verification => {
            let pressed = ev.kind == sdl_event::CONTROLLERBUTTONDOWN;
            if menu && pressed && ev.joy_button as i32 == game_controller::button::A && ctx.has_engine {
                if let Some(pointer) = pointer.as_deref_mut() {
                    out.push(pointer.position_action(ctx));
                }
            }
            let translated = if let Some(pointer) = pointer.as_deref_mut() {
                pointer.button(ev.joy_button as i32, pressed, menu)
            } else {
                game_controller::translate_button(ev.joy_button as i32, pressed)
            };
            if let Some((key, action)) = translated {
                out.push(Action::Cause { key, action, delta: 0.0 });
            }
        }
        sdl_event::WINDOWEVENT => translate_window(ctx, ev, verification, &mut out),
        _ => {}
    }
    out
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
///
/// `MouseSensitivityX`/`MouseSensitivityY` multiply the deltas here, on top of
/// the script-side `MouseSensitivity`; the defaults of `1.0` leave them exact.
pub fn relative_mouse(ctx: &Context, delta_x: i32, delta_y: i32) -> ActionList {
    relative_mouse_scaled(
        ctx,
        delta_x,
        delta_y,
        input_options::mouse_sensitivity_x(),
        input_options::mouse_sensitivity_y(),
    )
}

/// [`relative_mouse`] with explicit per-axis sensitivity multipliers.
pub fn relative_mouse_scaled(
    ctx: &Context,
    delta_x: i32,
    delta_y: i32,
    sensitivity_x: f32,
    sensitivity_y: f32,
) -> ActionList {
    let mut out = ActionList::new();
    if delta_x != 0 {
        out.push(Action::Cause {
            key: ik::MOUSE_X,
            action: ist::AXIS,
            delta: delta_x as f32 * ctx.mouse_scale_x * sensitivity_x,
        });
    }
    if delta_y != 0 {
        out.push(Action::Cause {
            key: ik::MOUSE_Y,
            action: ist::AXIS,
            delta: -(delta_y as f32) * ctx.mouse_scale_y * sensitivity_y,
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
        if !verification
            && matches!(
                event.kind,
                sdl_event::CONTROLLERDEVICEADDED | sdl_event::CONTROLLERDEVICEREMOVED
            )
        {
            // Hot-plug is device state, not input: it is neither recorded nor
            // translated. Unplugging releases everything the pad held.
            if game_controller::handle_device_event(event.kind, event.motion_x)
                == game_controller::DeviceChange::Closed
            {
                release_controller_inputs(backend)?;
            }
            continue;
        }
        let raw_joystick_shadowed = matches!(
            event.kind,
            sdl_event::JOYAXISMOTION
                | sdl_event::JOYBUTTONDOWN
                | sdl_event::JOYBUTTONUP
                | sdl_event::JOYHATMOTION
        ) && game_controller::active();
        if input_trace::recording()
            && event.kind != 0x200
            && event.kind != 0x100
            && !raw_joystick_shadowed
        {
            if let Some(now) = input_trace::gameplay_time(backend.game_time()?) {
                input_trace::record_event(now, &event);
            }
        }
        let context = backend.context()?;
        let actions = {
            let mut pointer = MENU_POINTER.lock().expect("menu pointer lock poisoned");
            translate_event_inner(&context, &event, verification, Some(&mut pointer), game_controller::menu_mode())
        };
        execute_actions(backend, &actions)?;
    }
    if !verification {
        let context = backend.context()?;
        let action = {
            let mut pointer = MENU_POINTER.lock().expect("menu pointer lock poisoned");
            let now = std::time::Instant::now();
            let dt = pointer.last_step.replace(now).map_or(0.0, |last| now.duration_since(last).as_secs_f32().min(0.1));
            if game_controller::active() && game_controller::menu_mode() && context.has_engine && !context.mouse_is_grabbed {
                pointer.step(&context, dt)
            } else {
                None
            }
        };
        if let Some(action) = action {
            backend.execute_action(action)?;
        }
    }
    Ok(())
}

/// Neutralizes every input the game controller layer can hold: axes to zero,
/// buttons released only when currently down.
#[cfg(feature = "sdl")]
pub(crate) fn release_controller_inputs<B: UpdateInputBackend>(
    backend: &mut B,
) -> Result<(), B::Error> {
    for key in game_controller::AXIS_KEYS {
        backend.cause_input(key, ist::AXIS, 0.0)?;
    }
    for key in game_controller::held_key_candidates() {
        if backend.key_down(key)? {
            backend.cause_input(key, ist::RELEASE, 0.0)?;
        }
    }
    let virtual_click = {
        let mut pointer = MENU_POINTER.lock().expect("menu pointer lock poisoned");
        let held = pointer.held.contains(&Some(ik::LEFT_MOUSE));
        *pointer = MenuPointer::default();
        held
    };
    if virtual_click && backend.key_down(ik::LEFT_MOUSE)? {
        backend.cause_input(ik::LEFT_MOUSE, ist::RELEASE, 0.0)?;
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
    *MENU_POINTER.lock().expect("menu pointer lock poisoned") = MenuPointer::default();
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
mod tests;
