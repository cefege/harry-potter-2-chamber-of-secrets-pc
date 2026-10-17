//! Normalized input-event adapter, ported from
//! `HarryPotter2/Unreal/SDLDrv/Inc/HP2InputEvents.h`.
//!
//! **Correction (caught on review): the real header's own "PREP ONLY
//! -- not yet wired into the viewport pump" comment is stale relative
//! to the actual C++, and an earlier version of this module doc
//! repeated that stale claim without cross-checking it against code
//! already read in this same session.** `USDLViewport`'s real,
//! live `UpdateInput` (`SDLViewport.cpp:318-480`) already calls these
//! exact functions at real call sites: the constructor fills
//! `KeysymMap` from `HP2MapKeysym` (`:84-85`), and the event loop
//! calls `HP2MakeKeyEvent` (`:345`), `HP2MakeTextEvent` (`:352`),
//! `HP2MakeMouseButtonEvent` (`:370`) and `HP2MakeWheelEvents`
//! (`:378`) directly, each immediately followed by `CauseInputEvent`/
//! `Engine->Key`. So the real cutover is *not* "design a new batch
//! SDL-poll FFI protocol" (an earlier version of this doc described
//! exactly that, also wrong) -- it is switching those five call sites
//! from the local inline C++ functions to `hp2_rs_*` equivalents,
//! one call at a time, keeping the exact same per-event, inline-in-
//! the-poll-loop shape the pump already has today. `CauseInputEvent`
//! itself (`:535-542`) calls `GetOuterUSDLClient()->Engine->
//! InputEvent(...)` -- a live `UEngine*` virtual dispatch -- so that
//! part stays C++ until Phase 5; only the pure mapping step ahead of
//! it moves. This makes the `#[repr(C)]` layout of `HP2InputEvent`
//! exactly matching `FHP2InputEvent` a live, present-tense
//! requirement, not a future-design nicety -- see that struct's own
//! doc comment below for the fix this review caught (an earlier
//! version omitted `#[repr(C)]` and used a `[u8;7]+len` shape that
//! did not match the real `ANSICHAR Text[8]` layout at all).
//!
//! `EInputKey`/`EInputAction` (`Engine/Inc/EngineClasses.h:129-401`) are
//! both plain, dense, zero-based C++ enums (`IK_None=0 .. IK_MAX=255`,
//! `IST_None=0 .. IST_MAX=5`), confirmed by reading every line of both
//! declarations directly, not by assuming default Rust enum numbering. Only
//! the subset of named values this file's functions and its oracle test
//! actually reference are named below; `Key`/`Action` stay plain `i32`
//! rather than a closed Rust enum because `HP2MakeTextEvent` legitimately
//! assigns `Key` an arbitrary Unicode codepoint value (e.g. 233 for U+00E9)
//! that has no corresponding named `IK_*` constant -- exactly mirroring the
//! real C++'s own `(EInputKey)Codepoint` cast. A `#[repr(i32)] enum EInputKey`
//! would be undefined behavior the moment a text event's arbitrary codepoint
//! doesn't match any declared variant.

/// `EInputKey` values this file's logic touches (`EngineClasses.h:131-369`).
#[allow(dead_code)]
pub mod ik {
    pub const NONE: i32 = 0;
    pub const LEFT_MOUSE: i32 = 1;
    pub const RIGHT_MOUSE: i32 = 2;
    pub const MIDDLE_MOUSE: i32 = 4;
    pub const BACKSPACE: i32 = 8;
    pub const TAB: i32 = 9;
    pub const ENTER: i32 = 13;
    pub const SHIFT: i32 = 16;
    pub const CTRL: i32 = 17;
    pub const ALT: i32 = 18;
    pub const PAUSE: i32 = 19;
    pub const CAPS_LOCK: i32 = 20;
    pub const ESCAPE: i32 = 27;
    pub const SPACE: i32 = 32;
    pub const PAGE_UP: i32 = 33;
    pub const PAGE_DOWN: i32 = 34;
    pub const END: i32 = 35;
    pub const HOME: i32 = 36;
    pub const LEFT: i32 = 37;
    pub const UP: i32 = 38;
    pub const RIGHT: i32 = 39;
    pub const DOWN: i32 = 40;
    pub const INSERT: i32 = 45;
    pub const DELETE: i32 = 46;
    pub const N0: i32 = 48;
    pub const N1: i32 = 49;
    pub const N2: i32 = 50;
    pub const N3: i32 = 51;
    pub const N4: i32 = 52;
    pub const N5: i32 = 53;
    pub const N6: i32 = 54;
    pub const N7: i32 = 55;
    pub const N8: i32 = 56;
    pub const N9: i32 = 57;
    pub const A: i32 = 65;
    pub const B: i32 = 66;
    pub const C: i32 = 67;
    pub const D: i32 = 68;
    pub const E: i32 = 69;
    pub const F: i32 = 70;
    pub const G: i32 = 71;
    pub const H: i32 = 72;
    pub const I: i32 = 73;
    pub const J: i32 = 74;
    pub const K: i32 = 75;
    pub const L: i32 = 76;
    pub const M: i32 = 77;
    pub const N: i32 = 78;
    pub const O: i32 = 79;
    pub const P: i32 = 80;
    pub const Q: i32 = 81;
    pub const R: i32 = 82;
    pub const S: i32 = 83;
    pub const T: i32 = 84;
    pub const U: i32 = 85;
    pub const V: i32 = 86;
    pub const W: i32 = 87;
    pub const X: i32 = 88;
    pub const Y: i32 = 89;
    pub const Z: i32 = 90;
    pub const NUM_PAD0: i32 = 96;
    pub const NUM_PAD1: i32 = 97;
    pub const NUM_PAD2: i32 = 98;
    pub const NUM_PAD3: i32 = 99;
    pub const NUM_PAD4: i32 = 100;
    pub const NUM_PAD5: i32 = 101;
    pub const NUM_PAD6: i32 = 102;
    pub const NUM_PAD7: i32 = 103;
    pub const NUM_PAD8: i32 = 104;
    pub const NUM_PAD9: i32 = 105;
    pub const GREY_STAR: i32 = 106;
    pub const GREY_PLUS: i32 = 107;
    pub const SEPARATOR: i32 = 108;
    pub const GREY_MINUS: i32 = 109;
    pub const NUM_PAD_PERIOD: i32 = 110;
    pub const GREY_SLASH: i32 = 111;
    pub const F1: i32 = 112;
    pub const F2: i32 = 113;
    pub const F3: i32 = 114;
    pub const F4: i32 = 115;
    pub const F5: i32 = 116;
    pub const F6: i32 = 117;
    pub const F7: i32 = 118;
    pub const F8: i32 = 119;
    pub const F9: i32 = 120;
    pub const F10: i32 = 121;
    pub const F11: i32 = 122;
    pub const F12: i32 = 123;
    pub const F13: i32 = 124;
    pub const F14: i32 = 125;
    pub const F15: i32 = 126;
    pub const F24: i32 = 135;
    pub const NUM_LOCK: i32 = 144;
    pub const SCROLL_LOCK: i32 = 145;
    pub const L_SHIFT: i32 = 160;
    pub const R_SHIFT: i32 = 161;
    pub const SEMICOLON: i32 = 186;
    pub const EQUALS: i32 = 187;
    pub const COMMA: i32 = 188;
    pub const MINUS: i32 = 189;
    pub const PERIOD: i32 = 190;
    pub const SLASH: i32 = 191;
    pub const TILDE: i32 = 192;
    pub const LEFT_BRACKET: i32 = 219;
    pub const BACKSLASH: i32 = 220;
    pub const RIGHT_BRACKET: i32 = 221;
    pub const SINGLE_QUOTE: i32 = 222;
    pub const MOUSE_W: i32 = 231;
    pub const MOUSE_WHEEL_UP: i32 = 236;
    pub const MOUSE_WHEEL_DOWN: i32 = 237;
}

/// `EInputAction` values (`EngineClasses.h:394-401`), all of them -- there
/// are only 6.
#[allow(dead_code)]
pub mod ist {
    pub const NONE: i32 = 0;
    pub const PRESS: i32 = 1;
    pub const HOLD: i32 = 2;
    pub const RELEASE: i32 = 3;
    pub const AXIS: i32 = 4;
    pub const MAX: i32 = 5;
}

/// `FHP2InputEvent` (`HP2InputEvents.h:37-43`): the normalized event shape,
/// exactly `#[repr(C)]` matching the real struct's layout -- `EInputKey
/// Key` and `EInputAction Action` are both plain C++ `enum`s (`int`-sized
/// by default, no explicit underlying type declared), `FLOAT Delta` is
/// `f32`, and `ANSICHAR Text[8]` is an 8-byte NUL-terminated buffer with
/// the terminator stored *in* the array -- not a separate length field,
/// which an earlier version of this struct had (`[u8;7]+text_len: u8`,
/// never `#[repr(C)]` at all): same total size by coincidence (8 bytes
/// either way) but a different byte-for-byte shape, caught on review
/// before this crossed FFI anywhere. `text`'s contents are only
/// meaningful for `SDL_TEXTINPUT`-sourced events; every other event
/// constructor leaves it zeroed, matching `HP2MakeNoneEvent`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HP2InputEvent {
    pub key: i32,
    pub action: i32,
    pub delta: f32,
    pub text: [u8; 8],
}

const _: () = assert!(std::mem::size_of::<HP2InputEvent>() == 20);
const _: () = assert!(std::mem::offset_of!(HP2InputEvent, key) == 0);
const _: () = assert!(std::mem::offset_of!(HP2InputEvent, action) == 4);
const _: () = assert!(std::mem::offset_of!(HP2InputEvent, delta) == 8);
const _: () = assert!(std::mem::offset_of!(HP2InputEvent, text) == 12);

/// `HP2MakeNoneEvent` (`HP2InputEvents.h:45-50`).
pub fn make_none_event() -> HP2InputEvent {
    HP2InputEvent {
        key: ik::NONE,
        action: ist::NONE,
        delta: 0.0,
        text: [0; 8],
    }
}

/// `HP2MapKeysym` (`HP2InputEvents.h:53-185`), mirroring
/// `USDLViewport::KeysymMap`. `sym` is an `SDL_Keycode` (`i32`); returns
/// `ik::NONE` for unmapped keysyms. Frozen quirks preserved verbatim from
/// the C++ (do not "fix" unilaterally -- see the real header's own note):
/// both CTRL keys collapse to `ik::CTRL`, both GUI/Command keys surface as
/// `ik::F24`, both ALT keys map to `ik::ALT`, and the Spanish enye key
/// (241) and the Apple international section-sign key (167) both map to
/// `ik::TILDE`.
pub fn map_keysym(sym: i32) -> i32 {
    // SDLK_* values below are SDL2's own keycode constants (ASCII-valued for
    // printable keys; `SDLK_SCANCODE_MASK | scancode` for the rest), copied
    // from `<SDL2/SDL_keycode.h>` to keep this slice SDL-header-free.
    const SDLK_RETURN: i32 = b'\r' as i32;
    const SDLK_ESCAPE: i32 = 27;
    const SDLK_BACKSPACE: i32 = 8;
    const SDLK_TAB: i32 = 9;
    const SDLK_SPACE: i32 = b' ' as i32;
    const SDLK_QUOTE: i32 = b'\'' as i32;
    const SDLK_COMMA: i32 = b',' as i32;
    const SDLK_MINUS: i32 = b'-' as i32;
    const SDLK_PERIOD: i32 = b'.' as i32;
    const SDLK_SLASH: i32 = b'/' as i32;
    const SDLK_0: i32 = b'0' as i32;
    const SDLK_1: i32 = b'1' as i32;
    const SDLK_2: i32 = b'2' as i32;
    const SDLK_3: i32 = b'3' as i32;
    const SDLK_4: i32 = b'4' as i32;
    const SDLK_5: i32 = b'5' as i32;
    const SDLK_6: i32 = b'6' as i32;
    const SDLK_7: i32 = b'7' as i32;
    const SDLK_8: i32 = b'8' as i32;
    const SDLK_9: i32 = b'9' as i32;
    const SDLK_SEMICOLON: i32 = b';' as i32;
    const SDLK_EQUALS: i32 = b'=' as i32;
    const SDLK_LEFTBRACKET: i32 = b'[' as i32;
    const SDLK_BACKSLASH: i32 = b'\\' as i32;
    const SDLK_RIGHTBRACKET: i32 = b']' as i32;
    const SDLK_BACKQUOTE: i32 = b'`' as i32;
    const SDLK_A: i32 = b'a' as i32;
    const SDLK_B: i32 = b'b' as i32;
    const SDLK_C: i32 = b'c' as i32;
    const SDLK_D: i32 = b'd' as i32;
    const SDLK_E: i32 = b'e' as i32;
    const SDLK_F: i32 = b'f' as i32;
    const SDLK_G: i32 = b'g' as i32;
    const SDLK_H: i32 = b'h' as i32;
    const SDLK_I: i32 = b'i' as i32;
    const SDLK_J: i32 = b'j' as i32;
    const SDLK_K: i32 = b'k' as i32;
    const SDLK_L: i32 = b'l' as i32;
    const SDLK_M: i32 = b'm' as i32;
    const SDLK_N: i32 = b'n' as i32;
    const SDLK_O: i32 = b'o' as i32;
    const SDLK_P: i32 = b'p' as i32;
    const SDLK_Q: i32 = b'q' as i32;
    const SDLK_R: i32 = b'r' as i32;
    const SDLK_S: i32 = b's' as i32;
    const SDLK_T: i32 = b't' as i32;
    const SDLK_U: i32 = b'u' as i32;
    const SDLK_V: i32 = b'v' as i32;
    const SDLK_W: i32 = b'w' as i32;
    const SDLK_X: i32 = b'x' as i32;
    const SDLK_Y: i32 = b'y' as i32;
    const SDLK_Z: i32 = b'z' as i32;
    const SDLK_DELETE: i32 = 127;
    // Scancode-derived keys: SDL2 defines `SDLK_SCANCODE_MASK = 1<<30` and
    // `SDLK_x = SDL_SCANCODE_x | SDLK_SCANCODE_MASK` for the rest. Values
    // below are the real `SDL_keycode.h` constants.
    const SDLK_CAPSLOCK: i32 = 0x4000_0039u32 as i32;
    const SDLK_F1: i32 = 0x4000_003Au32 as i32;
    const SDLK_F2: i32 = 0x4000_003Bu32 as i32;
    const SDLK_F3: i32 = 0x4000_003Cu32 as i32;
    const SDLK_F4: i32 = 0x4000_003Du32 as i32;
    const SDLK_F5: i32 = 0x4000_003Eu32 as i32;
    const SDLK_F6: i32 = 0x4000_003Fu32 as i32;
    const SDLK_F7: i32 = 0x4000_0040u32 as i32;
    const SDLK_F8: i32 = 0x4000_0041u32 as i32;
    const SDLK_F9: i32 = 0x4000_0042u32 as i32;
    const SDLK_F10: i32 = 0x4000_0043u32 as i32;
    const SDLK_F11: i32 = 0x4000_0044u32 as i32;
    const SDLK_F12: i32 = 0x4000_0045u32 as i32;
    const SDLK_PRINTSCREEN: i32 = 0x4000_0046u32 as i32;
    const SDLK_SCROLLLOCK: i32 = 0x4000_0047u32 as i32;
    const SDLK_PAUSE: i32 = 0x4000_0048u32 as i32;
    const SDLK_INSERT: i32 = 0x4000_0049u32 as i32;
    const SDLK_HOME: i32 = 0x4000_004Au32 as i32;
    const SDLK_PAGEUP: i32 = 0x4000_004Bu32 as i32;
    const SDLK_END: i32 = 0x4000_004Du32 as i32;
    const SDLK_PAGEDOWN: i32 = 0x4000_004Eu32 as i32;
    const SDLK_RIGHT: i32 = 0x4000_004Fu32 as i32;
    const SDLK_LEFT: i32 = 0x4000_0050u32 as i32;
    const SDLK_DOWN: i32 = 0x4000_0051u32 as i32;
    const SDLK_UP: i32 = 0x4000_0052u32 as i32;
    const SDLK_NUMLOCKCLEAR: i32 = 0x4000_0053u32 as i32;
    const SDLK_KP_DIVIDE: i32 = 0x4000_0054u32 as i32;
    const SDLK_KP_MULTIPLY: i32 = 0x4000_0055u32 as i32;
    const SDLK_KP_MINUS: i32 = 0x4000_0056u32 as i32;
    const SDLK_KP_PLUS: i32 = 0x4000_0057u32 as i32;
    const SDLK_KP_ENTER: i32 = 0x4000_0058u32 as i32;
    const SDLK_KP_1: i32 = 0x4000_0059u32 as i32;
    const SDLK_KP_2: i32 = 0x4000_005Au32 as i32;
    const SDLK_KP_3: i32 = 0x4000_005Bu32 as i32;
    const SDLK_KP_4: i32 = 0x4000_005Cu32 as i32;
    const SDLK_KP_5: i32 = 0x4000_005Du32 as i32;
    const SDLK_KP_6: i32 = 0x4000_005Eu32 as i32;
    const SDLK_KP_7: i32 = 0x4000_005Fu32 as i32;
    const SDLK_KP_8: i32 = 0x4000_0060u32 as i32;
    const SDLK_KP_9: i32 = 0x4000_0061u32 as i32;
    const SDLK_KP_0: i32 = 0x4000_0062u32 as i32;
    const SDLK_KP_PERIOD: i32 = 0x4000_0063u32 as i32;
    const SDLK_APPLICATION: i32 = 0x4000_0065u32 as i32;
    const SDLK_KP_EQUALS: i32 = 0x4000_0067u32 as i32;
    const SDLK_F13: i32 = 0x4000_0068u32 as i32;
    const SDLK_F14: i32 = 0x4000_0069u32 as i32;
    const SDLK_F15: i32 = 0x4000_006Au32 as i32;
    const SDLK_LCTRL: i32 = 0x4000_00E0u32 as i32;
    const SDLK_LSHIFT: i32 = 0x4000_00E1u32 as i32;
    const SDLK_LALT: i32 = 0x4000_00E2u32 as i32;
    const SDLK_LGUI: i32 = 0x4000_00E3u32 as i32;
    const SDLK_RCTRL: i32 = 0x4000_00E4u32 as i32;
    const SDLK_RSHIFT: i32 = 0x4000_00E5u32 as i32;
    const SDLK_RALT: i32 = 0x4000_00E6u32 as i32;
    const SDLK_RGUI: i32 = 0x4000_00E7u32 as i32;

    match sym {
        // Letters.
        SDLK_A => ik::A,
        SDLK_B => ik::B,
        SDLK_C => ik::C,
        SDLK_D => ik::D,
        SDLK_E => ik::E,
        SDLK_F => ik::F,
        SDLK_G => ik::G,
        SDLK_H => ik::H,
        SDLK_I => ik::I,
        SDLK_J => ik::J,
        SDLK_K => ik::K,
        SDLK_L => ik::L,
        SDLK_M => ik::M,
        SDLK_N => ik::N,
        SDLK_O => ik::O,
        SDLK_P => ik::P,
        SDLK_Q => ik::Q,
        SDLK_R => ik::R,
        SDLK_S => ik::S,
        SDLK_T => ik::T,
        SDLK_U => ik::U,
        SDLK_V => ik::V,
        SDLK_W => ik::W,
        SDLK_X => ik::X,
        SDLK_Y => ik::Y,
        SDLK_Z => ik::Z,

        // Digits and space.
        SDLK_0 => ik::N0,
        SDLK_1 => ik::N1,
        SDLK_2 => ik::N2,
        SDLK_3 => ik::N3,
        SDLK_4 => ik::N4,
        SDLK_5 => ik::N5,
        SDLK_6 => ik::N6,
        SDLK_7 => ik::N7,
        SDLK_8 => ik::N8,
        SDLK_9 => ik::N9,
        SDLK_SPACE => ik::SPACE,

        // TTY functions.
        SDLK_BACKSPACE => ik::BACKSPACE,
        SDLK_TAB => ik::TAB,
        SDLK_RETURN => ik::ENTER,
        SDLK_PAUSE => ik::PAUSE,
        SDLK_ESCAPE => ik::ESCAPE,
        SDLK_DELETE => ik::DELETE,
        SDLK_INSERT => ik::INSERT,

        // Modifiers (frozen quirks: Ctrl collapse, GUI as F24).
        SDLK_LSHIFT => ik::L_SHIFT,
        SDLK_RSHIFT => ik::R_SHIFT,
        SDLK_LCTRL => ik::CTRL,
        SDLK_RCTRL => ik::CTRL,
        SDLK_LGUI => ik::F24,
        SDLK_RGUI => ik::F24,
        SDLK_LALT => ik::ALT,
        SDLK_RALT => ik::ALT,

        // Special remaps.
        SDLK_BACKQUOTE => ik::TILDE,
        SDLK_QUOTE => ik::SINGLE_QUOTE,
        SDLK_SEMICOLON => ik::SEMICOLON,
        SDLK_COMMA => ik::COMMA,
        SDLK_PERIOD => ik::PERIOD,
        SDLK_SLASH => ik::SLASH,
        SDLK_BACKSLASH => ik::BACKSLASH,
        SDLK_LEFTBRACKET => ik::LEFT_BRACKET,
        SDLK_RIGHTBRACKET => ik::RIGHT_BRACKET,
        241 => ik::TILDE, // Spanish N key
        167 => ik::TILDE, // +/- section sign, Apple intl QWERTY

        // Function keys.
        SDLK_F1 => ik::F1,
        SDLK_F2 => ik::F2,
        SDLK_F3 => ik::F3,
        SDLK_F4 => ik::F4,
        SDLK_F5 => ik::F5,
        SDLK_F6 => ik::F6,
        SDLK_F7 => ik::F7,
        SDLK_F8 => ik::F8,
        SDLK_F9 => ik::F9,
        SDLK_F10 => ik::F10,
        SDLK_F11 => ik::F11,
        SDLK_F12 => ik::F12,
        SDLK_F13 => ik::F13,
        SDLK_F14 => ik::F14,
        SDLK_F15 => ik::F15,

        // Cursor control and motion.
        SDLK_HOME => ik::HOME,
        SDLK_LEFT => ik::LEFT,
        SDLK_UP => ik::UP,
        SDLK_RIGHT => ik::RIGHT,
        SDLK_DOWN => ik::DOWN,
        SDLK_PAGEUP => ik::PAGE_UP,
        SDLK_PAGEDOWN => ik::PAGE_DOWN,
        SDLK_END => ik::END,

        // Keypad.
        SDLK_KP_ENTER => ik::ENTER,
        SDLK_KP_0 => ik::NUM_PAD0,
        SDLK_KP_1 => ik::NUM_PAD1,
        SDLK_KP_2 => ik::NUM_PAD2,
        SDLK_KP_3 => ik::NUM_PAD3,
        SDLK_KP_4 => ik::NUM_PAD4,
        SDLK_KP_5 => ik::NUM_PAD5,
        SDLK_KP_6 => ik::NUM_PAD6,
        SDLK_KP_7 => ik::NUM_PAD7,
        SDLK_KP_8 => ik::NUM_PAD8,
        SDLK_KP_9 => ik::NUM_PAD9,
        SDLK_KP_MULTIPLY => ik::GREY_STAR,
        SDLK_KP_PLUS => ik::GREY_PLUS,
        SDLK_KP_EQUALS => ik::SEPARATOR,
        SDLK_KP_MINUS => ik::GREY_MINUS,
        SDLK_KP_PERIOD => ik::NUM_PAD_PERIOD,
        SDLK_KP_DIVIDE => ik::GREY_SLASH,

        // Other.
        SDLK_MINUS => ik::MINUS,
        SDLK_EQUALS => ik::EQUALS,
        SDLK_NUMLOCKCLEAR => ik::NUM_LOCK,
        SDLK_CAPSLOCK => ik::CAPS_LOCK,
        SDLK_SCROLLLOCK => ik::SCROLL_LOCK,

        _ => {
            // SDLK_APPLICATION, SDLK_PRINTSCREEN, SDLK_UNKNOWN (0), and
            // every other keysym the real switch's `default:` falls
            // through on, all land here exactly like the C++.
            let _ = (SDLK_APPLICATION, SDLK_PRINTSCREEN);
            ik::NONE
        }
    }
}

/// `HP2MakeKeyEvent` (`HP2InputEvents.h:189-195`).
pub fn make_key_event(sym: i32, pressed: bool) -> HP2InputEvent {
    let mut e = make_none_event();
    e.key = map_keysym(sym);
    e.action = if pressed { ist::PRESS } else { ist::RELEASE };
    e
}

/// `HP2MakeMouseButtonEvent` (`HP2InputEvents.h:200-212`). Only SDL buttons
/// 1/2/3 reach the engine; anything else is a suppressed `ik::NONE` event.
pub fn make_mouse_button_event(button: u8, pressed: bool) -> HP2InputEvent {
    let mut e = make_none_event();
    e.key = match button {
        1 => ik::LEFT_MOUSE,
        2 => ik::MIDDLE_MOUSE,
        3 => ik::RIGHT_MOUSE,
        _ => return e, // suppressed: ik::NONE + ist::NONE
    };
    e.action = if pressed { ist::PRESS } else { ist::RELEASE };
    e
}

/// `HP2MakeWheelEvents` (`HP2InputEvents.h:219-239`). Returns the number of
/// events written into `out` (0..=3): zero for a zero delta (matching the
/// real pump's `if (Event.wheel.y)` guard), otherwise an `ik::MOUSE_W` axis
/// event carrying the delta, then a synthesized press/release pair on
/// `ik::MOUSE_WHEEL_UP`/`ik::MOUSE_WHEEL_DOWN`.
pub fn make_wheel_events(wheel_y: i32, out: &mut [HP2InputEvent; 3]) -> i32 {
    if wheel_y == 0 {
        return 0;
    }

    out[0] = make_none_event();
    out[0].key = ik::MOUSE_W;
    out[0].action = ist::AXIS;
    out[0].delta = wheel_y as f32;

    let direction = if wheel_y < 0 {
        ik::MOUSE_WHEEL_DOWN
    } else {
        ik::MOUSE_WHEEL_UP
    };

    out[1] = make_none_event();
    out[1].key = direction;
    out[1].action = ist::PRESS;

    out[2] = make_none_event();
    out[2].key = direction;
    out[2].action = ist::RELEASE;

    3
}

/// `HP2Utf8Codepoint` (`HP2InputEvents.h:244-270`), mirroring
/// `ucs2_char_code_from_utf8()` in `SDLViewport.cpp` (BMP-only UCS2
/// semantics, including its silent acceptance of truncated-but-prefixed
/// sequences -- a 2/3-byte lead whose continuation byte(s) are simply
/// absent from `utf8` reads past the end via the same indexing the real
/// C++ does, relying on the NUL terminator or trailing byte at that offset
/// evaluating outside `0x80..0xC0` to fail the check; callers in this
/// crate always pass a longer-than-the-claimed-length NUL-terminated byte
/// slice so that read is always in-bounds, matching the C++'s own
/// reliance on a NUL-terminated `const char*`). `utf8` is `None` only for
/// the real C++'s `Utf8 == NULL` case (-1); a `Some(&[])` empty-but-
/// present string is *not* the same thing -- the real C++ would read
/// `Utf8[0]`, the string's own NUL terminator (value 0), and return `0`,
/// not `-1` (caught on review: an earlier version of this function
/// treated an empty slice the same as `None`, both returning `-1`, which
/// is wrong for the empty-string case specifically).
pub fn utf8_codepoint(utf8: Option<&[u8]>) -> i32 {
    let Some(utf8) = utf8 else {
        return -1;
    };
    let c0 = utf8.first().copied().unwrap_or(0);
    if c0 < 128 {
        return c0 as i32;
    }
    if c0 < 224 {
        let c1 = *utf8.get(1).unwrap_or(&0);
        if (128..192).contains(&c1) {
            return (((c0 & 31) as i32) << 6) | (c1 & 63) as i32;
        }
        return -1;
    }
    if c0 < 240 {
        let c1 = *utf8.get(1).unwrap_or(&0);
        if (128..192).contains(&c1) {
            let c2 = *utf8.get(2).unwrap_or(&0);
            if (128..192).contains(&c2) {
                return (((c0 & 15) as i32) << 12) | (((c1 & 63) as i32) << 6) | (c2 & 63) as i32;
            }
        }
        return -1;
    }
    -1
}

/// `HP2MakeTextEvent` (`HP2InputEvents.h:276-289`). `utf8_text` is `None`
/// for the C++'s `NULL` case.
pub fn make_text_event(utf8_text: Option<&[u8]>) -> HP2InputEvent {
    let mut e = make_none_event();
    let codepoint = utf8_codepoint(utf8_text);
    e.key = if codepoint >= 0 { codepoint } else { ik::NONE };
    if let Some(bytes) = utf8_text {
        // `e.text` is already zeroed by `make_none_event()`; copying only
        // the first `n` bytes (n <= 7, one byte of the 8-byte array always
        // held back) leaves `e.text[n]` as the in-band NUL terminator,
        // exactly matching the real C++'s own `Text[Index] = 0` after its
        // copy loop -- no separate length field.
        let n = bytes.iter().take(7).take_while(|&&b| b != 0).count();
        e.text[..n].copy_from_slice(&bytes[..n]);
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `Tests/InputContractTests.cpp::TestInputEventMapping`'s
    /// `KeyRows` table (`InputContractTests.cpp:580-602`) row for row,
    /// transcribed from the real C++ oracle test, registered as the
    /// `input_event_mapping` ctest.
    #[test]
    fn mirrors_the_real_cpp_keysym_mapping_table() {
        const SDLK_A: i32 = b'a' as i32;
        const SDLK_Z: i32 = b'z' as i32;
        const SDLK_9: i32 = b'9' as i32;
        const SDLK_SPACE: i32 = b' ' as i32;
        const SDLK_RETURN: i32 = b'\r' as i32;
        const SDLK_KP_ENTER: i32 = 0x4000_0058u32 as i32;
        const SDLK_LCTRL: i32 = 0x4000_00E0u32 as i32;
        const SDLK_RCTRL: i32 = 0x4000_00E4u32 as i32;
        const SDLK_LGUI: i32 = 0x4000_00E3u32 as i32;
        const SDLK_RGUI: i32 = 0x4000_00E7u32 as i32;
        const SDLK_LALT: i32 = 0x4000_00E2u32 as i32;
        const SDLK_BACKQUOTE: i32 = b'`' as i32;
        const SDLK_F15: i32 = 0x4000_006Au32 as i32;
        const SDLK_KP_DIVIDE: i32 = 0x4000_0054u32 as i32;
        const SDLK_NUMLOCKCLEAR: i32 = 0x4000_0053u32 as i32;
        const SDLK_APPLICATION: i32 = 0x4000_0065u32 as i32;
        const SDLK_UNKNOWN: i32 = 0;

        let rows: &[(i32, i32, &str)] = &[
            (SDLK_A, ik::A, "a"),
            (SDLK_Z, ik::Z, "z"),
            (SDLK_9, ik::N9, "9"),
            (SDLK_SPACE, ik::SPACE, "space"),
            (SDLK_RETURN, ik::ENTER, "return"),
            (SDLK_KP_ENTER, ik::ENTER, "kp_enter"),
            (SDLK_LCTRL, ik::CTRL, "lctrl-collapse"),
            (SDLK_RCTRL, ik::CTRL, "rctrl-collapse"),
            (SDLK_LGUI, ik::F24, "lgui-as-f24"),
            (SDLK_RGUI, ik::F24, "rgui-as-f24"),
            (SDLK_LALT, ik::ALT, "alt"),
            (SDLK_BACKQUOTE, ik::TILDE, "backquote"),
            (241, ik::TILDE, "spanish-enye"),
            (167, ik::TILDE, "apple-section-sign"),
            (SDLK_F15, ik::F15, "f15"),
            (SDLK_KP_DIVIDE, ik::GREY_SLASH, "kp_divide"),
            (SDLK_NUMLOCKCLEAR, ik::NUM_LOCK, "numlock"),
            (SDLK_APPLICATION, ik::NONE, "unmapped-application"),
            (SDLK_UNKNOWN, ik::NONE, "unknown"),
        ];
        for &(sym, expected, name) in rows {
            assert_eq!(map_keysym(sym), expected, "keysym '{name}' mismatch");
        }
    }

    /// `InputContractTests.cpp:611-622`.
    #[test]
    fn mirrors_the_real_cpp_key_event_normalization() {
        const SDLK_SPACE: i32 = b' ' as i32;
        const SDLK_Q: i32 = b'q' as i32;
        const SDLK_APPLICATION: i32 = 0x4000_0065u32 as i32;

        let press = make_key_event(SDLK_SPACE, true);
        assert_eq!(press.key, ik::SPACE);
        assert_eq!(press.action, ist::PRESS);
        assert_eq!(press.delta, 0.0);
        assert_eq!(press.text, [0; 8]);

        let release = make_key_event(SDLK_Q, false);
        assert_eq!(release.key, ik::Q);
        assert_eq!(release.action, ist::RELEASE);

        let suppressed = make_key_event(SDLK_APPLICATION, true);
        assert_eq!(suppressed.key, ik::NONE);
        assert_eq!(suppressed.action, ist::PRESS);
    }

    /// `InputContractTests.cpp:624-638`.
    #[test]
    fn mirrors_the_real_cpp_mouse_button_normalization() {
        let left = make_mouse_button_event(1, true);
        assert_eq!(left.key, ik::LEFT_MOUSE);
        assert_eq!(left.action, ist::PRESS);

        let middle = make_mouse_button_event(2, false);
        assert_eq!(middle.key, ik::MIDDLE_MOUSE);
        assert_eq!(middle.action, ist::RELEASE);

        let right = make_mouse_button_event(3, true);
        assert_eq!(right.key, ik::RIGHT_MOUSE);
        assert_eq!(right.action, ist::PRESS);

        let side = make_mouse_button_event(8, true);
        assert_eq!(side.key, ik::NONE);
        assert_eq!(side.action, ist::NONE);
    }

    /// `InputContractTests.cpp:640-659`.
    #[test]
    fn mirrors_the_real_cpp_wheel_sequence() {
        let mut events = [make_none_event(); 3];
        assert_eq!(make_wheel_events(0, &mut events), 0);

        assert_eq!(make_wheel_events(3, &mut events), 3);
        assert_eq!(events[0].key, ik::MOUSE_W);
        assert_eq!(events[0].action, ist::AXIS);
        assert_eq!(events[0].delta, 3.0);
        assert_eq!(events[1].key, ik::MOUSE_WHEEL_UP);
        assert_eq!(events[1].action, ist::PRESS);
        assert_eq!(events[2].key, ik::MOUSE_WHEEL_UP);
        assert_eq!(events[2].action, ist::RELEASE);

        assert_eq!(make_wheel_events(-2, &mut events), 3);
        assert_eq!(events[0].delta, -2.0);
        assert_eq!(events[1].key, ik::MOUSE_WHEEL_DOWN);
        assert_eq!(events[2].key, ik::MOUSE_WHEEL_DOWN);
    }

    /// `InputContractTests.cpp:661-678`.
    #[test]
    fn mirrors_the_real_cpp_text_event_normalization() {
        let ascii = make_text_event(Some(b"A"));
        assert_eq!(ascii.key, 65);
        assert_eq!(ascii.action, ist::NONE);
        assert_eq!(ascii.text, [b'A', 0, 0, 0, 0, 0, 0, 0]);

        let accent = make_text_event(Some(&[0xC3, 0xA9])); // U+00E9
        assert_eq!(accent.key, 233, "two-byte utf8 decode");

        let astral = make_text_event(Some(&[0xF0, 0x9F, 0x98, 0x80])); // astral plane
        assert_eq!(
            astral.key,
            ik::NONE,
            "astral plane must decode to IK_None like the UCS2 helper"
        );

        let malformed = make_text_event(Some(&[0xFF]));
        assert_eq!(malformed.key, ik::NONE);

        let truncated = make_text_event(Some(&[0xC3]));
        assert_eq!(truncated.key, ik::NONE);

        // Regression for a bug caught on review: an empty-but-present
        // string (`Some(&[])`, matching the real C++'s `Utf8Text == ""`)
        // must decode to codepoint 0 (reading the string's own NUL
        // terminator byte), not -1/IK_None -- -1 is reserved for `None`
        // (the real C++'s `Utf8Text == NULL`).
        let empty_but_present = make_text_event(Some(&[]));
        assert_eq!(
            empty_but_present.key, 0,
            "empty string must decode to codepoint 0, not IK_None"
        );
        let null = make_text_event(None);
        assert_eq!(null.key, ik::NONE, "NULL must decode to IK_None (via -1)");
    }

    #[test]
    fn make_none_event_matches_the_real_cpp_sentinel() {
        let e = make_none_event();
        assert_eq!(e.key, ik::NONE);
        assert_eq!(e.action, ist::NONE);
        assert_eq!(e.text, [0; 8]);
    }
}
