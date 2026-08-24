//! Input normalization ported from the SDL driver's frozen quirk tables.
//!
//! Sources (verbatim semantics):
//! - `HarryPotter2/Unreal/SDLDrv/Inc/HP2InputEvents.h` — the keysym map
//!   (Ctrl collapse, GUI-as-F24, tilde aliases), key/mouse/wheel/text
//!   normalization helpers, and the BMP-only UTF-8 codepoint decoder;
//! - `HarryPotter2/Unreal/SDLDrv/Src/SDLViewport.cpp` — `ReleaseAllInput`
//!   ascending-order teardown and the `GJoyAxisKeys` table.

use self::sdl::*;
use crate::input_keys as ik;
use ik::*;

/// SDL keycode constants needed by the frozen map. Scancode-derived keysyms
/// carry `(1 << 30) | scancode`.
pub mod sdl {
    pub const SDLK_SCANCODE_MASK: u32 = 1 << 30;

    pub const fn scancode_key(scancode: u32) -> u32 {
        scancode | SDLK_SCANCODE_MASK
    }

    pub const SDLK_UNKNOWN: u32 = 0;
    pub const SDLK_APPLICATION: u32 = scancode_key(101);
    pub const SDLK_BACKSPACE: u32 = 8;
    pub const SDLK_TAB: u32 = 9;
    pub const SDLK_RETURN: u32 = 13;
    pub const SDLK_ESCAPE: u32 = 27;
    pub const SDLK_SPACE: u32 = 32;
    pub const SDLK_DELETE: u32 = 127;

    pub const SDLK_CAPSLOCK: u32 = scancode_key(57);
    pub const SDLK_F1: u32 = scancode_key(58);
    pub const SDLK_F2: u32 = scancode_key(59);
    pub const SDLK_F3: u32 = scancode_key(60);
    pub const SDLK_F4: u32 = scancode_key(61);
    pub const SDLK_F5: u32 = scancode_key(62);
    pub const SDLK_F6: u32 = scancode_key(63);
    pub const SDLK_F7: u32 = scancode_key(64);
    pub const SDLK_F8: u32 = scancode_key(65);
    pub const SDLK_F9: u32 = scancode_key(66);
    pub const SDLK_F10: u32 = scancode_key(67);
    pub const SDLK_F11: u32 = scancode_key(68);
    pub const SDLK_F12: u32 = scancode_key(69);
    pub const SDLK_F13: u32 = scancode_key(104);
    pub const SDLK_F14: u32 = scancode_key(105);
    pub const SDLK_F15: u32 = scancode_key(106);
    pub const SDLK_SCROLLLOCK: u32 = scancode_key(71);
    pub const SDLK_PAUSE: u32 = scancode_key(72);
    pub const SDLK_INSERT: u32 = scancode_key(73);
    pub const SDLK_HOME: u32 = scancode_key(74);
    pub const SDLK_PAGEUP: u32 = scancode_key(75);
    pub const SDLK_PAGEDOWN: u32 = scancode_key(78);
    pub const SDLK_END: u32 = scancode_key(77);
    pub const SDLK_RIGHT: u32 = scancode_key(79);
    pub const SDLK_LEFT: u32 = scancode_key(80);
    pub const SDLK_DOWN: u32 = scancode_key(81);
    pub const SDLK_UP: u32 = scancode_key(82);
    pub const SDLK_NUMLOCKCLEAR: u32 = scancode_key(83);
    pub const SDLK_KP_DIVIDE: u32 = scancode_key(84);
    pub const SDLK_KP_MULTIPLY: u32 = scancode_key(85);
    pub const SDLK_KP_MINUS: u32 = scancode_key(86);
    pub const SDLK_KP_PLUS: u32 = scancode_key(87);
    pub const SDLK_KP_ENTER: u32 = scancode_key(88);
    // Keypad digits are scancodes 89..98 in order.
    pub const SDLK_KP_0: u32 = scancode_key(89);
    pub const SDLK_KP_1: u32 = scancode_key(90);
    pub const SDLK_KP_2: u32 = scancode_key(91);
    pub const SDLK_KP_3: u32 = scancode_key(92);
    pub const SDLK_KP_4: u32 = scancode_key(93);
    pub const SDLK_KP_5: u32 = scancode_key(94);
    pub const SDLK_KP_6: u32 = scancode_key(95);
    pub const SDLK_KP_7: u32 = scancode_key(96);
    pub const SDLK_KP_8: u32 = scancode_key(97);
    pub const SDLK_KP_9: u32 = scancode_key(98);
    pub const SDLK_KP_PERIOD: u32 = scancode_key(99);
    pub const SDLK_KP_EQUALS: u32 = scancode_key(103);
    pub const SDLK_LCTRL: u32 = scancode_key(224);
    pub const SDLK_LSHIFT: u32 = scancode_key(225);
    pub const SDLK_LALT: u32 = scancode_key(226);
    pub const SDLK_LGUI: u32 = scancode_key(227);
    pub const SDLK_RCTRL: u32 = scancode_key(228);
    pub const SDLK_RSHIFT: u32 = scancode_key(229);
    pub const SDLK_RALT: u32 = scancode_key(230);
    pub const SDLK_RGUI: u32 = scancode_key(231);

    // Printable punctuation keysyms are their ASCII code points.
    pub const SDLK_MINUS: u32 = b'-' as u32;
    pub const SDLK_EQUALS: u32 = b'=' as u32;
    pub const SDLK_BACKQUOTE: u32 = b'`' as u32;
    pub const SDLK_QUOTE: u32 = b'\'' as u32;
    pub const SDLK_SEMICOLON: u32 = b';' as u32;
    pub const SDLK_COMMA: u32 = b',' as u32;
    pub const SDLK_PERIOD: u32 = b'.' as u32;
    pub const SDLK_SLASH: u32 = b'/' as u32;
    pub const SDLK_BACKSLASH: u32 = b'\\' as u32;
    pub const SDLK_LEFTBRACKET: u32 = b'[' as u32;
    pub const SDLK_RIGHTBRACKET: u32 = b']' as u32;
}

// ASCII range bounds usable in match patterns.
const ASCII_LOWER_A: u32 = b'a' as u32;
const ASCII_LOWER_Z: u32 = b'z' as u32;
const ASCII_DIGIT_0: u32 = b'0' as u32;
const ASCII_DIGIT_9: u32 = b'9' as u32;

/// Unreal `EInputAction` values (`IST_*`), numeric order preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    None,
    Press,
    Hold,
    Release,
    Axis,
}

/// One normalized engine input event (`FHP2InputEvent` analogue). Text
/// events carry their payload in `text`; all other events leave it empty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputEvent {
    pub key: u8,
    pub action: InputAction,
    pub delta: f32,
    pub text: [u8; 8],
}

impl InputEvent {
    fn none() -> Self {
        Self {
            key: IK_NONE,
            action: InputAction::None,
            delta: 0.0,
            text: [0; 8],
        }
    }
}

/// Mirror of `USDLViewport::KeysymMap` / `HP2MapKeysym`: the frozen quirk
/// table (Ctrl collapse, GUI-as-F24, tilde aliases). Unmapped keysyms keep
/// `IK_NONE`.
pub fn map_keysym(sym: u32) -> u8 {
    match sym {
        // Letters.
        ASCII_LOWER_A..=ASCII_LOWER_Z => (sym - ASCII_LOWER_A) as u8 + IK_A,

        // Digits and space.
        ASCII_DIGIT_0..=ASCII_DIGIT_9 => (sym - ASCII_DIGIT_0) as u8 + IK_0,
        SDLK_SPACE => IK_SPACE,

        // TTY functions.
        SDLK_BACKSPACE => IK_BACKSPACE,
        SDLK_TAB => IK_TAB,
        SDLK_RETURN => IK_ENTER,
        SDLK_PAUSE => IK_PAUSE,
        SDLK_ESCAPE => IK_ESCAPE,
        SDLK_DELETE => IK_DELETE,
        SDLK_INSERT => IK_INSERT,

        // Modifiers (frozen quirks: Ctrl collapse, GUI as F24).
        SDLK_LSHIFT => IK_LSHIFT,
        SDLK_RSHIFT => IK_RSHIFT,
        SDLK_LCTRL | SDLK_RCTRL => IK_CTRL,
        SDLK_LGUI | SDLK_RGUI => IK_F24,
        SDLK_LALT | SDLK_RALT => IK_ALT,

        // Special remaps.
        SDLK_BACKQUOTE => IK_Tilde,
        SDLK_QUOTE => IK_SingleQuote,
        SDLK_SEMICOLON => IK_Semicolon,
        SDLK_COMMA => IK_Comma,
        SDLK_PERIOD => IK_Period,
        SDLK_SLASH => IK_Slash,
        SDLK_BACKSLASH => IK_Backslash,
        SDLK_LEFTBRACKET => IK_LeftBracket,
        SDLK_RIGHTBRACKET => IK_RightBracket,
        241 => IK_Tilde, // Spanish N key
        167 => IK_Tilde, // +/- section sign, Apple intl QWERTY

        // Function keys.
        SDLK_F1 => IK_F1,
        SDLK_F2 => IK_F2,
        SDLK_F3 => IK_F3,
        SDLK_F4 => IK_F4,
        SDLK_F5 => IK_F5,
        SDLK_F6 => IK_F6,
        SDLK_F7 => IK_F7,
        SDLK_F8 => IK_F8,
        SDLK_F9 => IK_F9,
        SDLK_F10 => IK_F10,
        SDLK_F11 => IK_F11,
        SDLK_F12 => IK_F12,
        SDLK_F13 => IK_F13,
        SDLK_F14 => IK_F14,
        SDLK_F15 => IK_F15,

        // Cursor control and motion.
        SDLK_HOME => IK_HOME,
        SDLK_LEFT => IK_LEFT,
        SDLK_UP => IK_UP,
        SDLK_RIGHT => IK_RIGHT,
        SDLK_DOWN => IK_DOWN,
        SDLK_PAGEUP => IK_PAGE_UP,
        SDLK_PAGEDOWN => IK_PAGE_DOWN,
        SDLK_END => IK_END,

        // Keypad.
        SDLK_KP_ENTER => IK_ENTER,
        SDLK_KP_0 => IK_NumPad0,
        SDLK_KP_1 => IK_NumPad1,
        SDLK_KP_2 => IK_NumPad2,
        SDLK_KP_3 => IK_NumPad3,
        SDLK_KP_4 => IK_NumPad4,
        SDLK_KP_5 => IK_NumPad5,
        SDLK_KP_6 => IK_NumPad6,
        SDLK_KP_7 => IK_NumPad7,
        SDLK_KP_8 => IK_NumPad8,
        SDLK_KP_9 => IK_NumPad9,
        SDLK_KP_MULTIPLY => IK_GreyStar,
        SDLK_KP_PLUS => IK_GreyPlus,
        SDLK_KP_EQUALS => IK_Separator,
        SDLK_KP_MINUS => IK_GreyMinus,
        SDLK_KP_PERIOD => IK_NumPadPeriod,
        SDLK_KP_DIVIDE => IK_GreySlash,

        // Other.
        SDLK_MINUS => IK_Minus,
        SDLK_EQUALS => IK_Equals,
        SDLK_NUMLOCKCLEAR => IK_NUM_LOCK,
        SDLK_CAPSLOCK => IK_CAPS_LOCK,
        SDLK_SCROLLLOCK => IK_SCROLL_LOCK,

        _ => IK_NONE,
    }
}

/// `HP2MakeKeyEvent`: unmapped keysyms keep `IK_NONE` while carrying the
/// press/release suppression marker.
pub fn make_key_event(sym: u32, pressed: bool) -> InputEvent {
    let mut event = InputEvent::none();
    event.key = map_keysym(sym);
    event.action = if pressed {
        InputAction::Press
    } else {
        InputAction::Release
    };
    event
}

/// `HP2MakeMouseButtonEvent`: only buttons 1/2/3 reach the engine; anything
/// else normalizes to a suppressed `IK_NONE`/`IST_None` event.
pub fn make_mouse_button_event(button: u8, pressed: bool) -> InputEvent {
    let mut event = InputEvent::none();
    event.key = match button {
        1 => IK_LEFT_MOUSE,
        2 => IK_MIDDLE_MOUSE,
        3 => IK_RIGHT_MOUSE,
        _ => return event,
    };
    event.action = if pressed {
        InputAction::Press
    } else {
        InputAction::Release
    };
    event
}

/// `HP2MakeWheelEvents`: one `IK_MouseW` axis event carrying the wheel
/// delta, then a synthesized press/release pair on the wheel direction.
/// Empty for a zero delta, matching the pump's guard.
pub fn make_wheel_events(wheel_y: i32) -> Vec<InputEvent> {
    if wheel_y == 0 {
        return Vec::new();
    }
    let direction = if wheel_y < 0 {
        IK_MOUSE_WHEEL_DOWN
    } else {
        IK_MOUSE_WHEEL_UP
    };
    let mut axis = InputEvent::none();
    axis.key = IK_MOUSE_W;
    axis.action = InputAction::Axis;
    axis.delta = wheel_y as f32;
    let mut press = InputEvent::none();
    press.key = direction;
    press.action = InputAction::Press;
    let mut release = InputEvent::none();
    release.key = direction;
    release.action = InputAction::Release;
    vec![axis, press, release]
}

/// Port of `HP2Utf8Codepoint`: first UTF-8 codepoint of a text payload,
/// BMP-only UCS2 semantics (astral planes are rejected); `None` when
/// malformed or truncated. Missing continuation bytes read as NUL, matching
/// the C helper's buffer semantics.
pub fn utf8_first_codepoint(payload: &[u8]) -> Option<u32> {
    let byte_at = |index: usize| -> u8 { payload.get(index).copied().unwrap_or(0) };
    let c0 = byte_at(0);
    if c0 < 128 {
        return Some(u32::from(c0));
    }
    if c0 < 224 {
        let c1 = byte_at(1);
        if (128..192).contains(&c1) {
            return Some((u32::from(c0 & 31) << 6) | u32::from(c1 & 63));
        }
        return None;
    }
    if c0 < 240 {
        let c1 = byte_at(1);
        if (128..192).contains(&c1) {
            let c2 = byte_at(2);
            if (128..192).contains(&c2) {
                return Some(
                    (u32::from(c0 & 15) << 12) | (u32::from(c1 & 63) << 6) | u32::from(c2 & 63),
                );
            }
        }
        return None;
    }
    None
}

/// `HP2MakeTextEvent`: the engine routes these through console key-typing,
/// so the action stays `IST_None` while `key` carries the first decoded
/// codepoint (`IK_NONE` when malformed). Up to 7 payload bytes are copied.
pub fn make_text_event(payload: &[u8]) -> InputEvent {
    let mut event = InputEvent::none();
    event.key = match utf8_first_codepoint(payload) {
        Some(codepoint) if codepoint <= u32::from(u8::MAX) => codepoint as u8,
        _ => IK_NONE,
    };
    let copied = payload.len().min(7);
    event.text[..copied].copy_from_slice(&payload[..copied]);
    event.text[copied] = 0;
    event
}

/// Joystick axes in the exact order the engine's input contracts expect
/// (`GJoyAxisKeys`, `USDLClient::MaxJoystickAxes` = 8).
pub const JOY_AXIS_KEYS: [u8; 8] = [
    IK_JOY_X,
    IK_JOY_Y,
    IK_JOY_Z,
    IK_JOY_R,
    IK_JOY_U,
    IK_JOY_V,
    IK_UNKNOWN_EA,
    IK_UNKNOWN_EB,
];

/// Pure form of `USDLViewport::ReleaseAllInput`: held keys release first in
/// ascending `IK` order (so bindings observe a deterministic sequence),
/// joystick axes neutralize next, then the mouse axes. `is_held` answers
/// for each key whether the key-down table currently latches it.
pub fn release_all_sequence(is_held: impl Fn(u8) -> bool) -> Vec<InputEvent> {
    let mut events = Vec::new();
    for key in 0..IK_MAX {
        if is_held(key) {
            let mut event = InputEvent::none();
            event.key = key;
            event.action = InputAction::Release;
            events.push(event);
        }
    }
    for &axis in JOY_AXIS_KEYS
        .iter()
        .chain([IK_MOUSE_X, IK_MOUSE_Y, IK_MOUSE_W].iter())
    {
        let mut event = InputEvent::none();
        event.key = axis;
        event.action = InputAction::Axis;
        events.push(event);
    }
    events
}
