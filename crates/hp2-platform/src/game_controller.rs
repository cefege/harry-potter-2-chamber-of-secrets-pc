//! SDL GameController layer: standardized gamepads mapped onto the existing
//! UE1 joystick input keys, so the stock `DefUser.ini` `Joy*` bindings (and
//! any rebinding in the Controls page) apply unchanged.
//!
//! The original `SDLViewport::UpdateInput` only understood raw
//! `SDL_Joystick` axes/buttons, whose layout differs per device (on a modern
//! pad the right stick lands on the unbound `JoyZ`/`JoyR` and the triggers on
//! the look axes). When a device is a recognized game controller this layer
//! opens it and the raw `SDL_JOY*` events are ignored in its favour:
//!
//! | Controller input | UE1 key |
//! |---|---|
//! | left stick X / Y | `JoyX` / `JoyY` (scaled by `ScaleJBX`/`ScaleJBY`) |
//! | right stick X / Y | `JoyU` / `JoyV` (times `GamepadLookSensitivity`) |
//! | left / right trigger | `JoyZ` / `JoyR` axes, and `Joy11` / `Joy12` past half pull |
//! | X, A, B, Y | `Joy1` (Fire), `Joy2` (Jump), `Joy3` (AltFire), `Joy4` (Duck) |
//! | left / right shoulder | `Joy5` / `Joy6` |
//! | Back | `Joy7` |
//! | Start | `Escape` (pause menu) |
//! | left / right stick click | `Joy9` / `Joy10` |
//! | D-pad | `JoyPovUp` / `JoyPovDown` / `JoyPovLeft` / `JoyPovRight` |
//! In menus, the left stick and D-pad move the pointer, A clicks, and B
//! sends Escape; stick and D-pad joystick events continue to reach the game.

//!
//! Dead zone is the existing `JoystickDeadZone`. Opening honours the
//! original joystick selection (`JoystickNumber`) at start-up and hot-plugs
//! the first recognized controller afterwards.

use crate::input_event::{ik, ist};
use crate::joystick_input::map_joystick_axis;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// `SDL_GameControllerAxis`.
pub mod axis {
    pub const LEFT_X: i32 = 0;
    pub const LEFT_Y: i32 = 1;
    pub const RIGHT_X: i32 = 2;
    pub const RIGHT_Y: i32 = 3;
    pub const TRIGGER_LEFT: i32 = 4;
    pub const TRIGGER_RIGHT: i32 = 5;
}

/// `SDL_GameControllerButton`.
pub mod button {
    pub const A: i32 = 0;
    pub const B: i32 = 1;
    pub const X: i32 = 2;
    pub const Y: i32 = 3;
    pub const BACK: i32 = 4;
    pub const GUIDE: i32 = 5;
    pub const START: i32 = 6;
    pub const LEFT_STICK: i32 = 7;
    pub const RIGHT_STICK: i32 = 8;
    pub const LEFT_SHOULDER: i32 = 9;
    pub const RIGHT_SHOULDER: i32 = 10;
    pub const DPAD_UP: i32 = 11;
    pub const DPAD_DOWN: i32 = 12;
    pub const DPAD_LEFT: i32 = 13;
    pub const DPAD_RIGHT: i32 = 14;
}

/// `SDL_EventType` values of the controller events.
pub mod event {
    pub const AXISMOTION: u32 = 0x650;
    pub const BUTTONDOWN: u32 = 0x651;
    pub const BUTTONUP: u32 = 0x652;
    pub const DEVICEADDED: u32 = 0x653;
    pub const DEVICEREMOVED: u32 = 0x654;
}

/// `IK_JoyPovUp..IK_JoyPovRight` (`Actor.uc`, 0xF0..0xF3).
pub const JOY_POV_UP: i32 = 240;
pub const JOY_POV_DOWN: i32 = 241;
pub const JOY_POV_LEFT: i32 = 242;
pub const JOY_POV_RIGHT: i32 = 243;

/// Raw trigger value (0..=32767) above which the trigger counts as a pressed
/// button, and below which a pressed trigger releases.
const TRIGGER_PRESS: i32 = 16384;
const TRIGGER_RELEASE: i32 = 8192;
/// `Joy11` / `Joy12`, zero-based offsets from `IK_Joy1`.
const TRIGGER_BUTTON_OFFSET: [i32; 2] = [10, 11];

static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOWED: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(true);
static TRIGGERS_DOWN: AtomicU8 = AtomicU8::new(0);
static MENU_MODE: AtomicBool = AtomicBool::new(false);

/// Sets whether controller buttons and pointer movement use menu controls.
pub fn set_menu_mode(enabled: bool) {
    MENU_MODE.store(enabled, Ordering::Release);
}

/// Returns the current menu input mode.
pub fn menu_mode() -> bool {
    MENU_MODE.load(Ordering::Acquire)
}

/// Replaces complete UE joystick key names with the controller's button names.
pub fn button_labels(text: &str) -> Option<String> {
    fn name(token: &str) -> Option<&'static str> {
        Some(match token {
            "Joy1" => "X", "Joy2" => "A", "Joy3" => "B", "Joy4" => "Y",
            "Joy5" => "LB", "Joy6" => "RB", "Joy7" => "Back",
            "Joy9" => "L3", "Joy10" => "R3", "Joy11" => "LT", "Joy12" => "RT",
            "JoyPovUp" => "D-pad Up", "JoyPovDown" => "D-pad Down",
            "JoyPovLeft" => "D-pad Left", "JoyPovRight" => "D-pad Right",
            _ => return None,
        })
    }
    let mut result = String::with_capacity(text.len());
    let mut changed = false;
    let mut start = 0;
    for (i, ch) in text.char_indices().chain(std::iter::once((text.len(), ' '))) {
        if !ch.is_alphanumeric() && ch != '_' {
            let token = &text[start..i];
            if let Some(label) = name(token) {
                result.push_str(label);
                changed = true;
            } else {
                result.push_str(token);
            }
            if i < text.len() {
                result.push(ch);
            }
            start = i + ch.len_utf8();
        }
    }
    changed.then_some(result)
}

/// Whether a game controller is open; raw `SDL_JOY*` events are ignored then.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Acquire)
}

/// Key a controller button drives, or `None` for unmapped buttons.
pub fn button_key(button: i32) -> Option<i32> {
    Some(match button {
        button::X => ik::JOY1,
        button::A => ik::JOY1 + 1,
        button::B => ik::JOY1 + 2,
        button::Y => ik::JOY1 + 3,
        button::LEFT_SHOULDER => ik::JOY1 + 4,
        button::RIGHT_SHOULDER => ik::JOY1 + 5,
        button::BACK => ik::JOY1 + 6,
        button::START => ik::ESCAPE,
        button::LEFT_STICK => ik::JOY1 + 8,
        button::RIGHT_STICK => ik::JOY1 + 9,
        button::DPAD_UP => JOY_POV_UP,
        button::DPAD_DOWN => JOY_POV_DOWN,
        button::DPAD_LEFT => JOY_POV_LEFT,
        button::DPAD_RIGHT => JOY_POV_RIGHT,
        _ => return None,
    })
}

/// Every key this layer can hold down, for release on unplug.
pub fn held_key_candidates() -> impl Iterator<Item = i32> {
    (0..=button::DPAD_RIGHT)
        .filter_map(button_key)
        .chain(TRIGGER_BUTTON_OFFSET.iter().map(|offset| ik::JOY1 + offset))
}

/// The raw axis keys this layer drives, for zeroing on unplug.
pub const AXIS_KEYS: [i32; 6] = [
    ik::JOY_X,
    ik::JOY_Y,
    ik::JOY_Z,
    ik::JOY_R,
    ik::JOY_U,
    ik::JOY_V,
];

/// One `CauseInputEvent(key, action, delta)`.
pub type KeyEvent = (i32, i32, f32);

/// Up to two events (an analog axis plus a digital trigger transition).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisEvents {
    pub items: [KeyEvent; 2],
    pub len: usize,
}

impl AxisEvents {
    pub fn iter(&self) -> impl Iterator<Item = KeyEvent> + '_ {
        self.items[..self.len].iter().copied()
    }
}

/// Translates a controller button; `None` when unmapped.
pub fn translate_button(button: i32, pressed: bool) -> Option<(i32, i32)> {
    button_key(button).map(|key| (key, if pressed { ist::PRESS } else { ist::RELEASE }))
}

/// Translates a controller axis. `raw` is the signed 16-bit SDL value;
/// `look_sensitivity` scales the right stick after the dead zone. Trigger
/// transitions keep their last digital state process-wide, like the C++
/// `LastJoyHat` field.
pub fn translate_axis(
    axis: i32,
    raw: i32,
    dead_zone: f32,
    scale_x: f32,
    scale_y: f32,
    look_sensitivity: f32,
) -> AxisEvents {
    let (joy_axis, trigger) = match axis {
        axis::LEFT_X => (0, None),
        axis::LEFT_Y => (1, None),
        axis::TRIGGER_LEFT => (2, Some(0)),
        axis::TRIGGER_RIGHT => (3, Some(1)),
        axis::RIGHT_X => (4, None),
        axis::RIGHT_Y => (5, None),
        _ => return AxisEvents::default(),
    };
    let mut out = AxisEvents::default();
    if let Some((key, mut value)) = map_joystick_axis(joy_axis, raw, dead_zone, scale_x, scale_y) {
        if joy_axis >= 4 {
            value *= look_sensitivity;
        }
        out.items[0] = (key, ist::AXIS, value);
        out.len = 1;
    }
    if let Some(index) = trigger {
        let bit = 1u8 << index;
        let down = TRIGGERS_DOWN.load(Ordering::Relaxed) & bit != 0;
        let now_down = if down { raw >= TRIGGER_RELEASE } else { raw >= TRIGGER_PRESS };
        if now_down != down {
            if now_down {
                TRIGGERS_DOWN.fetch_or(bit, Ordering::Relaxed);
            } else {
                TRIGGERS_DOWN.fetch_and(!bit, Ordering::Relaxed);
            }
            out.items[out.len] = (
                ik::JOY1 + TRIGGER_BUTTON_OFFSET[index],
                if now_down { ist::PRESS } else { ist::RELEASE },
                0.0,
            );
            out.len += 1;
        }
    }
    out
}

/// What a device event did to the open controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceChange {
    None,
    Opened,
    /// The open controller was unplugged; the caller releases its inputs.
    Closed,
}

/// Records whether the client asked for joysticks at all (`UseJoystick`).
pub fn set_allowed(allowed: bool) {
    ALLOWED.store(allowed, Ordering::Release);
}

/// Records the `UseGameController` option. Disabling closes the controller.
/// Returns whether the open controller changed.
#[cfg(feature = "sdl")]
pub fn set_enabled(enabled: bool) -> DeviceChange {
    ENABLED.store(enabled, Ordering::Release);
    if enabled {
        if open_first_available() { DeviceChange::Opened } else { DeviceChange::None }
    } else if close() {
        DeviceChange::Closed
    } else {
        DeviceChange::None
    }
}

/// Whether game controllers are both allowed and enabled.
pub fn permitted() -> bool {
    ALLOWED.load(Ordering::Acquire) && ENABLED.load(Ordering::Acquire)
}

#[cfg(feature = "sdl")]
mod ffi {
    use std::ffi::c_void;
    unsafe extern "C" {
        pub fn SDL_InitSubSystem(flags: u32) -> i32;
        pub fn SDL_NumJoysticks() -> i32;
        pub fn SDL_IsGameController(index: i32) -> i32;
        pub fn SDL_GameControllerOpen(index: i32) -> *mut c_void;
        pub fn SDL_GameControllerClose(controller: *mut c_void);
        pub fn SDL_GameControllerGetJoystick(controller: *mut c_void) -> *mut c_void;
        pub fn SDL_JoystickInstanceID(joystick: *mut c_void) -> i32;
        pub fn SDL_GameControllerEventState(state: i32) -> i32;
    }
}

#[cfg(feature = "sdl")]
const SDL_INIT_GAMECONTROLLER: u32 = 0x0000_2000;
#[cfg(feature = "sdl")]
const SDL_ENABLE: i32 = 1;

#[cfg(feature = "sdl")]
struct Open {
    controller: usize,
    instance_id: i32,
}

#[cfg(feature = "sdl")]
static OPEN: std::sync::Mutex<Option<Open>> = std::sync::Mutex::new(None);

#[cfg(feature = "sdl")]
fn open_index(index: i32) -> bool {
    let mut slot = OPEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot.is_some() {
        return false;
    }
    // SAFETY: every call is valid at any time after SDL start-up; the opened
    // handle is only stored, queried for its instance id, and closed by
    // `close` below.
    unsafe {
        if ffi::SDL_InitSubSystem(SDL_INIT_GAMECONTROLLER) < 0 {
            return false;
        }
        if index < 0 || index >= ffi::SDL_NumJoysticks() || ffi::SDL_IsGameController(index) == 0 {
            return false;
        }
        let controller = ffi::SDL_GameControllerOpen(index);
        if controller.is_null() {
            return false;
        }
        ffi::SDL_GameControllerEventState(SDL_ENABLE);
        let instance_id = ffi::SDL_JoystickInstanceID(ffi::SDL_GameControllerGetJoystick(controller));
        *slot = Some(Open { controller: controller as usize, instance_id });
    }
    TRIGGERS_DOWN.store(0, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Release);
    true
}

/// Opens the controller at the client's configured joystick index (the
/// original `JoystickNumber` selection), if permitted and recognized.
#[cfg(feature = "sdl")]
pub fn open_configured(joystick_number: i32) -> bool {
    permitted() && open_index(joystick_number)
}

/// Opens the first recognized controller, if permitted.
#[cfg(feature = "sdl")]
pub fn open_first_available() -> bool {
    if !permitted() || active() {
        return false;
    }
    // SAFETY: plain SDL query after start-up.
    let count = unsafe {
        if ffi::SDL_InitSubSystem(SDL_INIT_GAMECONTROLLER) < 0 {
            return false;
        }
        ffi::SDL_NumJoysticks()
    };
    (0..count).any(open_index)
}

/// Closes the open controller; returns whether one was open.
#[cfg(feature = "sdl")]
pub fn close() -> bool {
    let mut slot = OPEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(open) = slot.take() else { return false };
    // SAFETY: `controller` came from `SDL_GameControllerOpen` and was just
    // taken out of the slot, so it is closed exactly once.
    unsafe { ffi::SDL_GameControllerClose(open.controller as *mut _) };
    ACTIVE.store(false, Ordering::Release);
    TRIGGERS_DOWN.store(0, Ordering::Relaxed);
    true
}

/// Applies a `SDL_CONTROLLERDEVICEADDED`/`REMOVED` event. `which` is the
/// device index for additions and the instance id for removals.
#[cfg(feature = "sdl")]
pub fn handle_device_event(kind: u32, which: i32) -> DeviceChange {
    match kind {
        event::DEVICEADDED => {
            if open_first_available_at(which) { DeviceChange::Opened } else { DeviceChange::None }
        }
        event::DEVICEREMOVED => {
            let matches = OPEN
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|open| open.instance_id == which);
            if matches && close() { DeviceChange::Closed } else { DeviceChange::None }
        }
        _ => DeviceChange::None,
    }
}

#[cfg(feature = "sdl")]
fn open_first_available_at(index: i32) -> bool {
    permitted() && !active() && open_index(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        TRIGGERS_DOWN.store(0, Ordering::Relaxed);
    }

    #[test]
    fn sticks_map_to_the_stock_joystick_axes() {
        let left = translate_axis(axis::LEFT_X, 16384, 0.0, 1.0, 1.0, 3.0);
        assert_eq!(left.iter().collect::<Vec<_>>(), vec![(ik::JOY_X, ist::AXIS, 0.5)]);
        let left_y = translate_axis(axis::LEFT_Y, -16384, 0.0, 1.0, 2.0, 3.0);
        assert_eq!(left_y.iter().collect::<Vec<_>>(), vec![(ik::JOY_Y, ist::AXIS, -1.0)]);
        // Right stick: JoyU/JoyV, look sensitivity applied after the dead zone.
        let right = translate_axis(axis::RIGHT_X, 16384, 0.0, 1.0, 1.0, 2.0);
        assert_eq!(right.iter().collect::<Vec<_>>(), vec![(ik::JOY_U, ist::AXIS, 1.0)]);
        let right_y = translate_axis(axis::RIGHT_Y, 16384, 0.0, 1.0, 1.0, 1.0);
        assert_eq!(right_y.iter().collect::<Vec<_>>(), vec![(ik::JOY_V, ist::AXIS, 0.5)]);
        // Inside the dead zone the stick is exactly zero regardless of sensitivity.
        let dead = translate_axis(axis::RIGHT_X, 1000, 0.2, 1.0, 1.0, 5.0);
        assert_eq!(dead.iter().collect::<Vec<_>>(), vec![(ik::JOY_U, ist::AXIS, 0.0)]);
        assert_eq!(translate_axis(99, 1, 0.0, 1.0, 1.0, 1.0).len, 0);
    }

    #[test]
    fn triggers_are_axes_with_hysteretic_buttons() {
        reset();
        let low = translate_axis(axis::TRIGGER_LEFT, 5000, 0.0, 1.0, 1.0, 1.0);
        assert_eq!(low.len, 1);
        let press = translate_axis(axis::TRIGGER_LEFT, 20000, 0.0, 1.0, 1.0, 1.0);
        assert_eq!(press.len, 2);
        assert_eq!(press.items[0].0, ik::JOY_Z);
        assert_eq!(press.items[1], (ik::JOY1 + 10, ist::PRESS, 0.0));
        // Between the thresholds nothing changes.
        assert_eq!(translate_axis(axis::TRIGGER_LEFT, 12000, 0.0, 1.0, 1.0, 1.0).len, 1);
        let release = translate_axis(axis::TRIGGER_LEFT, 4000, 0.0, 1.0, 1.0, 1.0);
        assert_eq!(release.items[1], (ik::JOY1 + 10, ist::RELEASE, 0.0));
        let right = translate_axis(axis::TRIGGER_RIGHT, 30000, 0.0, 1.0, 1.0, 1.0);
        assert_eq!((right.items[0].0, right.items[1].0), (ik::JOY_R, ik::JOY1 + 11));
        reset();
    }

    #[test]
    fn buttons_hit_the_default_bound_joy_keys() {
        assert_eq!(translate_button(button::X, true), Some((ik::JOY1, ist::PRESS)));
        assert_eq!(translate_button(button::A, true), Some((ik::JOY1 + 1, ist::PRESS)));
        assert_eq!(translate_button(button::B, false), Some((ik::JOY1 + 2, ist::RELEASE)));
        assert_eq!(translate_button(button::Y, true), Some((ik::JOY1 + 3, ist::PRESS)));
        assert_eq!(translate_button(button::START, true), Some((ik::ESCAPE, ist::PRESS)));
        assert_eq!(translate_button(button::DPAD_LEFT, true), Some((JOY_POV_LEFT, ist::PRESS)));
        assert_eq!(translate_button(button::GUIDE, true), None);
        assert!(held_key_candidates().all(|key| (0..ik::MAX).contains(&key)));
    }
}
