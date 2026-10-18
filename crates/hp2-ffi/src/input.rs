//! FFI exports for `hp2_platform::input_pump`: the per-event translation of
//! `USDLViewport::UpdateInput` (`SDLViewport.cpp`), the `ReleaseAllInput`
//! stepper, the relative-mouse branch, and the stalest-viewport pick in
//! `USDLClient::Tick` (`SDLClient.cpp`). Also the SDL mouse-state, grab-state
//! and joystick calls the C++ shell still makes.
//!
//! The C++ shell keeps every `SDL_*` poll, every `UObject` field write and
//! every engine call. Rust holds no state between calls: each export takes a
//! snapshot and returns the side effects as `FfiInputAction`s in the order the
//! shell must execute them.
//!
//! Mirror layouts are pinned by the `const` asserts below and by the matching
//! `static_assert`s in `SDLDrv.h`/`SDLViewport.cpp`.

use hp2_ffi_support::guard;
use hp2_platform::input_pump as pump;

#[cfg(feature = "platform-sdl")]
use std::ffi::c_void;

/// One polled `SDL_Event`, flattened by the C++ shell. Mirrors
/// `hp2_platform::input_pump::SdlEvent`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiSdlEvent {
    pub kind: u32,
    pub key_sym: i32,
    pub text: [u8; 32],
    pub motion_x: i32,
    pub motion_y: i32,
    pub motion_state: u32,
    pub button: u8,
    pub wheel_y: i32,
    pub joy_axis: u8,
    pub joy_axis_value: i32,
    pub joy_button: u8,
    pub joy_hat: u8,
    pub window_event: u8,
    pub window_data1: i32,
    pub window_data2: i32,
    pub window_flags: u32,
}

const _: () = assert!(std::mem::size_of::<FfiSdlEvent>() == 84);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, kind) == 0);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, key_sym) == 4);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, text) == 8);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, motion_x) == 40);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, motion_y) == 44);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, motion_state) == 48);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, button) == 52);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, wheel_y) == 56);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, joy_axis) == 60);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, joy_axis_value) == 64);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, joy_button) == 68);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, joy_hat) == 69);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, window_event) == 70);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, window_data1) == 72);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, window_data2) == 76);
const _: () = assert!(std::mem::offset_of!(FfiSdlEvent, window_flags) == 80);

impl From<FfiSdlEvent> for pump::SdlEvent {
    fn from(e: FfiSdlEvent) -> Self {
        pump::SdlEvent {
            kind: e.kind,
            key_sym: e.key_sym,
            text: e.text,
            motion_x: e.motion_x,
            motion_y: e.motion_y,
            motion_state: e.motion_state,
            button: e.button,
            wheel_y: e.wheel_y,
            joy_axis: e.joy_axis,
            joy_axis_value: e.joy_axis_value,
            joy_button: e.joy_button,
            joy_hat: e.joy_hat,
            window_event: e.window_event,
            window_data1: e.window_data1,
            window_data2: e.window_data2,
            window_flags: e.window_flags,
        }
    }
}

/// Per-event snapshot of the `USDLViewport`/`USDLClient` fields the pump
/// reads. Mirrors `hp2_platform::input_pump::Context`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiInputContext {
    pub mouse_is_grabbed: bool,
    pub lost_grab: bool,
    pub capture_mouse: bool,
    pub has_window: bool,
    pub has_engine: bool,
    pub has_actor: bool,
    pub joy_hat_folds_to_buttons: bool,
    pub mouse_scale_x: f32,
    pub mouse_scale_y: f32,
    pub size_x: i32,
    pub size_y: i32,
    pub joystick_dead_zone: f32,
    pub scale_jbx: f32,
    pub scale_jby: f32,
    pub last_joy_hat: i32,
}

const _: () = assert!(std::mem::size_of::<FfiInputContext>() == 40);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, mouse_is_grabbed) == 0);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, lost_grab) == 1);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, capture_mouse) == 2);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, has_window) == 3);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, has_engine) == 4);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, has_actor) == 5);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, joy_hat_folds_to_buttons) == 6);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, mouse_scale_x) == 8);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, mouse_scale_y) == 12);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, size_x) == 16);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, size_y) == 20);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, joystick_dead_zone) == 24);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, scale_jbx) == 28);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, scale_jby) == 32);
const _: () = assert!(std::mem::offset_of!(FfiInputContext, last_joy_hat) == 36);

impl From<FfiInputContext> for pump::Context {
    fn from(c: FfiInputContext) -> Self {
        pump::Context {
            mouse_is_grabbed: c.mouse_is_grabbed,
            lost_grab: c.lost_grab,
            capture_mouse: c.capture_mouse,
            has_window: c.has_window,
            has_engine: c.has_engine,
            has_actor: c.has_actor,
            joy_hat_folds_to_buttons: c.joy_hat_folds_to_buttons,
            mouse_scale_x: c.mouse_scale_x,
            mouse_scale_y: c.mouse_scale_y,
            size_x: c.size_x,
            size_y: c.size_y,
            joystick_dead_zone: c.joystick_dead_zone,
            scale_jbx: c.scale_jbx,
            scale_jby: c.scale_jby,
            last_joy_hat: c.last_joy_hat,
        }
    }
}

/// Which side effect an `FfiInputAction` asks the C++ shell to perform.
/// Variant values are pinned by `static_assert`s in `SDLViewport.cpp`.
// The variant names are C globals in hp2_rs.h (cbindgen emits them unscoped and
// the C++ shell's switch and static_assert use them by name), so the shared
// `InputAction` prefix stays on purpose.
#[allow(clippy::enum_variant_names)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiInputActionKind {
    /// `CauseInputEvent(key, action, delta)`.
    InputActionCause = 0,
    /// `ClientEngine->Key(this, key)`.
    InputActionEngineKey = 1,
    /// `Engine->MousePosition(this, buttons, x, y)`.
    InputActionEngineMousePosition = 2,
    /// `UpdateMouseGrabState(flag)`; `flag` is the capture value.
    InputActionUpdateMouseGrab = 3,
    /// `LostGrab = flag`.
    InputActionSetLostGrab = 4,
    /// `LastJoyHat = key`.
    InputActionSetLastJoyHat = 5,
    /// `ReleaseAllInput()`; the shell runs the `release_all_*` stepper.
    InputActionReleaseAll = 6,
    /// `ResizeViewport(BLIT_NoWindowChange, width, height, INDEX_NONE)`.
    InputActionResizeViewport = 7,
    /// Release-all only: `Input->KeyDown(key)`. The answer is passed as
    /// `held` to the next `hp2_rs_input_release_all_next` call.
    InputActionQueryKeyDown = 8,
}

/// One side effect for the C++ executor. Fields not used by `kind` are zero.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiInputAction {
    pub kind: FfiInputActionKind,
    pub key: i32,
    pub action: i32,
    pub delta: f32,
    pub buttons: u32,
    pub x: f32,
    pub y: f32,
    pub width: i32,
    pub height: i32,
    pub flag: bool,
}

const _: () = assert!(std::mem::size_of::<FfiInputAction>() == 40);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, kind) == 0);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, key) == 4);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, action) == 8);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, delta) == 12);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, buttons) == 16);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, x) == 20);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, y) == 24);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, width) == 28);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, height) == 32);
const _: () = assert!(std::mem::offset_of!(FfiInputAction, flag) == 36);

impl FfiInputAction {
    const fn blank(kind: FfiInputActionKind) -> Self {
        FfiInputAction {
            kind,
            key: 0,
            action: 0,
            delta: 0.0,
            buttons: 0,
            x: 0.0,
            y: 0.0,
            width: 0,
            height: 0,
            flag: false,
        }
    }
}

impl From<pump::Action> for FfiInputAction {
    fn from(action: pump::Action) -> Self {
        match action {
            pump::Action::Cause { key, action, delta } => FfiInputAction {
                key,
                action,
                delta,
                ..Self::blank(FfiInputActionKind::InputActionCause)
            },
            pump::Action::EngineKey { key } => FfiInputAction {
                key,
                ..Self::blank(FfiInputActionKind::InputActionEngineKey)
            },
            pump::Action::EngineMousePosition { buttons, x, y } => FfiInputAction {
                buttons,
                x,
                y,
                ..Self::blank(FfiInputActionKind::InputActionEngineMousePosition)
            },
            pump::Action::UpdateMouseGrab { capture } => FfiInputAction {
                flag: capture,
                ..Self::blank(FfiInputActionKind::InputActionUpdateMouseGrab)
            },
            pump::Action::SetLostGrab(value) => FfiInputAction {
                flag: value,
                ..Self::blank(FfiInputActionKind::InputActionSetLostGrab)
            },
            pump::Action::SetLastJoyHat(key) => FfiInputAction {
                key,
                ..Self::blank(FfiInputActionKind::InputActionSetLastJoyHat)
            },
            pump::Action::ReleaseAll => Self::blank(FfiInputActionKind::InputActionReleaseAll),
            pump::Action::ResizeViewport { width, height } => FfiInputAction {
                width,
                height,
                ..Self::blank(FfiInputActionKind::InputActionResizeViewport)
            },
        }
    }
}

impl From<pump::ReleaseStep> for FfiInputAction {
    fn from(step: pump::ReleaseStep) -> Self {
        match step {
            pump::ReleaseStep::QueryKeyDown(key) => FfiInputAction {
                key,
                ..Self::blank(FfiInputActionKind::InputActionQueryKeyDown)
            },
            pump::ReleaseStep::Cause { key, action, delta } => FfiInputAction {
                key,
                action,
                delta,
                ..Self::blank(FfiInputActionKind::InputActionCause)
            },
            pump::ReleaseStep::SetLastJoyHat(key) => FfiInputAction {
                key,
                ..Self::blank(FfiInputActionKind::InputActionSetLastJoyHat)
            },
        }
    }
}

/// `hp2_platform::input_pump::ReleasePhase` as a plain code, so C++ can hand
/// the cursor back without creating an invalid Rust enum value.
const PHASE_KEYS: u32 = 0;
const PHASE_JOY_AXES: u32 = 1;
const PHASE_JOY_HAT: u32 = 2;
const PHASE_MOUSE: u32 = 3;
const PHASE_DONE: u32 = 4;

/// Iteration state of `ReleaseAllInput`, owned by the C++ shell for the
/// duration of one release sequence. Mirrors
/// `hp2_platform::input_pump::ReleaseAllCursor`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiReleaseAllCursor {
    pub phase: u32,
    pub index: i32,
    pub awaiting_key: bool,
}

const _: () = assert!(std::mem::size_of::<FfiReleaseAllCursor>() == 12);
const _: () = assert!(std::mem::offset_of!(FfiReleaseAllCursor, phase) == 0);
const _: () = assert!(std::mem::offset_of!(FfiReleaseAllCursor, index) == 4);
const _: () = assert!(std::mem::offset_of!(FfiReleaseAllCursor, awaiting_key) == 8);

impl From<pump::ReleaseAllCursor> for FfiReleaseAllCursor {
    fn from(c: pump::ReleaseAllCursor) -> Self {
        let phase = match c.phase {
            pump::ReleasePhase::Keys => PHASE_KEYS,
            pump::ReleasePhase::JoyAxes => PHASE_JOY_AXES,
            pump::ReleasePhase::JoyHat => PHASE_JOY_HAT,
            pump::ReleasePhase::Mouse => PHASE_MOUSE,
            pump::ReleasePhase::Done => PHASE_DONE,
        };
        FfiReleaseAllCursor {
            phase,
            index: c.index,
            awaiting_key: c.awaiting_key,
        }
    }
}

impl FfiReleaseAllCursor {
    /// `None` when `phase` is not one of the codes above.
    fn to_cursor(self) -> Option<pump::ReleaseAllCursor> {
        let phase = match self.phase {
            PHASE_KEYS => pump::ReleasePhase::Keys,
            PHASE_JOY_AXES => pump::ReleasePhase::JoyAxes,
            PHASE_JOY_HAT => pump::ReleasePhase::JoyHat,
            PHASE_MOUSE => pump::ReleasePhase::Mouse,
            PHASE_DONE => pump::ReleasePhase::Done,
            _ => return None,
        };
        Some(pump::ReleaseAllCursor {
            phase,
            index: self.index,
            awaiting_key: self.awaiting_key,
        })
    }
}

/// One non-null `USDLClient::Viewports` entry as seen by `Tick`. Mirrors
/// `hp2_platform::input_pump::RepaintCandidate`. `best_minus_candidate` is
/// `FTime::operator-` between the current victim and this viewport (0 when there
/// is no victim yet), so the comparison stays exact in C++ fixed-point time.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FfiRepaintCandidate {
    pub realtime: bool,
    pub size_x: i32,
    pub size_y: i32,
    pub best_minus_candidate: f32,
}

const _: () = assert!(std::mem::size_of::<FfiRepaintCandidate>() == 16);
const _: () = assert!(std::mem::offset_of!(FfiRepaintCandidate, realtime) == 0);
const _: () = assert!(std::mem::offset_of!(FfiRepaintCandidate, size_x) == 4);
const _: () = assert!(std::mem::offset_of!(FfiRepaintCandidate, size_y) == 8);
const _: () = assert!(std::mem::offset_of!(FfiRepaintCandidate, best_minus_candidate) == 12);

impl From<FfiRepaintCandidate> for pump::RepaintCandidate {
    fn from(c: FfiRepaintCandidate) -> Self {
        pump::RepaintCandidate {
            realtime: c.realtime,
            size_x: c.size_x,
            size_y: c.size_y,
            best_minus_candidate: c.best_minus_candidate,
        }
    }
}

/// Writes `actions` to `out` in order and returns the count.
///
/// # Safety
/// `out` must have room for `pump::MAX_ACTIONS` writable `FfiInputAction`s.
unsafe fn write_actions(actions: pump::ActionList, out: *mut FfiInputAction) -> i32 {
    let mut count = 0usize;
    for action in actions.iter() {
        // SAFETY: the caller provides `MAX_ACTIONS` slots and an `ActionList`
        // holds at most `MAX_ACTIONS` entries.
        unsafe { out.add(count).write(action.into()) };
        count += 1;
    }
    count as i32
}

/// Translates one polled `SDL_Event` (the body of the `while(SDL_PollEvent)`
/// switch in `USDLViewport::UpdateInput`, `SDLViewport.cpp:351-497`).
/// Returns the number of actions written to `out_actions`.
///
/// # Safety
/// `ctx` and `ev` must point to initialized values for the duration of the
/// call. `out_actions` must point to `pump::MAX_ACTIONS` writable
/// `FfiInputAction` slots.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_translate_event(
    ctx: *const FfiInputContext,
    ev: *const FfiSdlEvent,
    out_actions: *mut FfiInputAction,
) -> i32 {
    guard("hp2_rs_input_translate_event", 0, move || unsafe {
        let ctx: pump::Context = (*ctx).into();
        let ev: pump::SdlEvent = (*ev).into();
        write_actions(pump::translate_event(&ctx, &ev), out_actions)
    })
}

/// The relative-mouse branch of `USDLViewport::UpdateInput`
/// (`SDLViewport.cpp:357-365`). The shell calls this only when
/// `MouseIsGrabbed && SdlWindow`, after `SDL_GetRelativeMouseState`.
/// Returns the number of actions written to `out_actions`.
///
/// # Safety
/// `ctx` must point to an initialized value. `out_actions` must point to
/// `pump::MAX_ACTIONS` writable `FfiInputAction` slots.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_relative_mouse(
    ctx: *const FfiInputContext,
    delta_x: i32,
    delta_y: i32,
    out_actions: *mut FfiInputAction,
) -> i32 {
    guard("hp2_rs_input_relative_mouse", 0, move || unsafe {
        let ctx: pump::Context = (*ctx).into();
        write_actions(pump::relative_mouse(&ctx, delta_x, delta_y), out_actions)
    })
}

/// Starts a `ReleaseAllInput` sequence. `has_input` is `Input != NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_input_release_all_begin(has_input: bool) -> FfiReleaseAllCursor {
    guard(
        "hp2_rs_input_release_all_begin",
        FfiReleaseAllCursor {
            phase: PHASE_DONE,
            index: 0,
            awaiting_key: false,
        },
        move || pump::release_all_begin(has_input).into(),
    )
}

/// Advances a `ReleaseAllInput` sequence by one step. `held` answers the
/// previous `InputActionQueryKeyDown`. Returns `true` and writes `out_action`
/// when a step is produced; returns `false` when the sequence is finished or
/// `cursor` holds an unknown phase code.
///
/// # Safety
/// `cursor` must point to a writable `FfiReleaseAllCursor` previously returned
/// by `hp2_rs_input_release_all_begin`. `out_action` must point to one
/// writable `FfiInputAction`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_input_release_all_next(
    cursor: *mut FfiReleaseAllCursor,
    held: bool,
    out_action: *mut FfiInputAction,
) -> bool {
    guard("hp2_rs_input_release_all_next", false, move || unsafe {
        let Some(mut state) = (*cursor).to_cursor() else {
            return false;
        };
        let step = pump::release_all_next(&mut state, held);
        *cursor = state.into();
        match step {
            Some(step) => {
                *out_action = step.into();
                true
            }
            None => false,
        }
    })
}

/// Whether `USDLClient::Tick` replaces its repaint victim with `candidate`
/// (`SDLClient.cpp`, victim fold). `has_best` says whether a victim already
/// exists. The C++ loop folds one viewport at a time, so no array crosses the
/// boundary.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_input_repaint_replaces_best(
    has_best: bool,
    candidate: FfiRepaintCandidate,
) -> bool {
    guard("hp2_rs_input_repaint_replaces_best", false, move || {
        pump::repaint_replaces_best(has_best, pump::RepaintCandidate::from(candidate))
    })
}

/// Replaces `USDLViewport::GetViewportButtonFlags` (`SDLViewport.cpp:544-548`).
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_platform_get_viewport_button_flags() -> u32 {
    guard("hp2_rs_platform_get_viewport_button_flags", 0, || {
        hp2_platform::mouse_state::get_viewport_button_flags()
    })
}

/// Replaces `USDLViewport::GetMouseState` (`SDLViewport.cpp:550-556`).
/// Writes the scaled `X`/`Y` into `out_x`/`out_y`, returns the button
/// flags (matching the real `DWORD` return of
/// `GetViewportButtonFlags`, reused here since `GetMouseState`'s own
/// `Buttons` out-param carries the identical value).
///
/// # Safety
/// `out_x`/`out_y` must be valid, aligned, writable pointers for the
/// duration of this call.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_get_mouse_state(
    mouse_scale_x: f32,
    mouse_scale_y: f32,
    out_x: *mut f32,
    out_y: *mut f32,
) -> u32 {
    guard("hp2_rs_platform_get_mouse_state", 0, move || unsafe {
        let (x, y, buttons) =
            hp2_platform::mouse_state::get_mouse_state(mouse_scale_x, mouse_scale_y);
        *out_x = x;
        *out_y = y;
        buttons
    })
}

/// Replaces `USDLViewport::UpdateMouseGrabState` (`SDLViewport.cpp:558-579`).
/// `sdl_window` may be null (matches the real C++'s `if (SdlWindow)`
/// guard). Writes the new `bWindowsMouseAvailable` into
/// `out_windows_mouse_available`; returns the new `MouseIsGrabbed`.
/// Caller stores both back into its own `USDLViewport` fields.
///
/// # Safety
/// `sdl_window`, if non-null, must be a valid `SDL_Window*` for the
/// duration of this call. `out_windows_mouse_available` must be a
/// valid, aligned, writable pointer.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_update_mouse_grab_state(
    sdl_window: *mut c_void,
    capture: bool,
    mouse_is_grabbed: bool,
    out_windows_mouse_available: *mut bool,
) -> bool {
    guard(
        "hp2_rs_platform_update_mouse_grab_state",
        mouse_is_grabbed,
        move || unsafe {
            let (new_grabbed, new_windows_mouse_available) =
                hp2_platform::mouse_state::update_mouse_grab_state(
                    sdl_window as *mut hp2_platform::mouse_state::SdlWindow,
                    capture,
                    mouse_is_grabbed,
                );
            *out_windows_mouse_available = new_windows_mouse_available;
            new_grabbed
        },
    )
}

/// Replaces `USDLClient::OpenJoystick` (`SDLClient.cpp:234-258`),
/// minus its own `CloseJoystick()` call at the start -- the real C++
/// caller still makes that call itself (now also FFI-forwarded, see
/// `hp2_rs_platform_close_joystick` below) before calling this.
/// Returns the opaque `SDL_Joystick*` handle on success (the caller
/// stores it in its own `Joystick` field, to hand back to
/// `hp2_rs_platform_close_joystick` later), or null on any failure --
/// `out_buttons`/`out_hats`/`out_axes`/`out_hat_folds_to_buttons` are
/// left unwritten in that case, matching the real C++'s own `return 0`
/// leaving `JoyButtons` etc. untouched.
///
/// # Safety
/// `out_buttons`/`out_hats`/`out_axes`/`out_hat_folds_to_buttons` must
/// be valid, aligned, writable pointers for the duration of this call.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_open_joystick(
    joystick_number: i32,
    ignore_hat: bool,
    out_buttons: *mut i32,
    out_hats: *mut i32,
    out_axes: *mut i32,
    out_hat_folds_to_buttons: *mut bool,
) -> *mut c_void {
    guard(
        "hp2_rs_platform_open_joystick",
        std::ptr::null_mut(),
        move || unsafe {
            let Some((handle, buttons, hats, axes, hat_folds_to_buttons)) =
                hp2_platform::joystick::open_joystick(joystick_number, ignore_hat)
            else {
                return std::ptr::null_mut();
            };
            *out_buttons = buttons;
            *out_hats = hats;
            *out_axes = axes;
            *out_hat_folds_to_buttons = hat_folds_to_buttons;
            handle as *mut c_void
        },
    )
}

/// Replaces the SDL half of `USDLClient::CloseJoystick`
/// (`SDLClient.cpp:261-274`); the caller zeroes its own
/// `JoyButtons`/`JoyHats`/`JoyAxes`/`JoyHatFoldsToButtons`/`Joystick`
/// fields, which Rust does not own. `joystick` may be null, matching
/// the real C++'s own `if (Joystick)` guard.
///
/// # Safety
/// `joystick`, if non-null, must be a valid, still-open
/// `SDL_Joystick*` obtained from `hp2_rs_platform_open_joystick` and
/// not already closed.
#[cfg(feature = "platform-sdl")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_platform_close_joystick(joystick: *mut c_void) {
    guard("hp2_rs_platform_close_joystick", (), move || unsafe {
        hp2_platform::joystick::close_joystick(joystick as *mut hp2_platform::joystick::SdlJoystick)
    })
}
