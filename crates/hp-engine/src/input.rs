//! Input dispatch: the recorded-script feeder and the
//! `CauseInputEvent → UEngine::InputEvent` chain.
//!
//! Key names and numeric values mirror `EInputKey`
//! (`HarryPotter2/Unreal/Engine/Inc/EngineClasses.h:130+`) and actions
//! mirror `EInputAction` (`IST_None..IST_Axis`). Script grammar is the
//! `Tests/Fixtures/input_script_smoke.txt` fixture format.

use crate::error::{EngineError, Result};

/// `EInputKey` values used by bindings and scripts (`IK_*`).
pub mod key {
    pub const NONE: u8 = 0;
    pub const LEFT_MOUSE: u8 = 1;
    pub const RIGHT_MOUSE: u8 = 2;
    pub const CANCEL: u8 = 3;
    pub const MIDDLE_MOUSE: u8 = 4;
    pub const BACKSPACE: u8 = 8;
    pub const TAB: u8 = 9;
    pub const ENTER: u8 = 13;
    pub const SHIFT: u8 = 16;
    pub const CTRL: u8 = 17;
    pub const ALT: u8 = 18;
    pub const PAUSE: u8 = 19;
    pub const CAPS_LOCK: u8 = 20;
    pub const ESCAPE: u8 = 27;
    pub const SPACE: u8 = 32;
    pub const PAGE_UP: u8 = 33;
    pub const PAGE_DOWN: u8 = 34;
    pub const END: u8 = 35;
    pub const HOME: u8 = 36;
    pub const LEFT: u8 = 37;
    pub const UP: u8 = 38;
    pub const RIGHT: u8 = 39;
    pub const DOWN: u8 = 40;
    pub const SELECT: u8 = 41;
    pub const PRINT: u8 = 42;
    pub const EXECUTE: u8 = 43;
    pub const PRINT_SCRN: u8 = 44;
    pub const INSERT: u8 = 45;
    pub const DELETE: u8 = 46;
    pub const HELP: u8 = 47;

    /// `IK_0 .. IK_9`.
    pub const fn digit(d: u8) -> u8 {
        48 + d
    }
    /// `IK_A .. IK_Z`.
    pub const fn letter(c: u8) -> u8 {
        65 + c
    }

    pub const NUM_PAD_0: u8 = 96;
    pub const GREY_STAR: u8 = 106;
    pub const GREY_PLUS: u8 = 107;
    pub const SEPARATOR: u8 = 108;
    pub const GREY_MINUS: u8 = 109;
    pub const NUM_PAD_PERIOD: u8 = 110;
    pub const GREY_SLASH: u8 = 111;
    /// `IK_F1 .. IK_F12` are 112..123; F13..F24 continue to 135.
    pub const fn function(f: u8) -> u8 {
        111 + f
    }
    pub const TILDE: u8 = 192;
    pub const MOUSE_WHEEL_UP: u8 = 236;
    pub const MOUSE_WHEEL_DOWN: u8 = 237;
    pub const MOUSE_W: u8 = 231;
    pub const NUM_LOCK: u8 = 144;
}

/// `EInputAction` (`IST_*`).
pub mod action {
    pub const NONE: u8 = 0;
    pub const PRESS: u8 = 1;
    pub const HOLD: u8 = 2;
    pub const RELEASE: u8 = 3;
    pub const AXIS: u8 = 4;
}

/// Resolve a binding/script key name (the `IK_` prefix omitted) to its
/// `EInputKey` value. Case-insensitive. Ported from the pure-mapping rows
/// of `Tests/InputContractTests.cpp:577+` plus the names the smoke fixture
/// and `[Engine.Input]` bindings use.
pub fn key_from_name(name: &str) -> Option<u8> {
    use key::*;
    let folded: String = name.to_ascii_lowercase();
    Some(match folded.as_str() {
        "none" => NONE,
        "leftmouse" => LEFT_MOUSE,
        "rightmouse" => RIGHT_MOUSE,
        "cancel" => CANCEL,
        "middlemouse" => MIDDLE_MOUSE,
        "backspace" => BACKSPACE,
        "tab" => TAB,
        "enter" | "return" | "kp_enter" => ENTER,
        "shift" => SHIFT,
        "ctrl" | "lctrl" | "rctrl" => CTRL,
        "alt" | "lalt" | "ralt" => ALT,
        "pause" => PAUSE,
        "capslock" => CAPS_LOCK,
        "escape" => ESCAPE,
        "space" => SPACE,
        "pageup" => PAGE_UP,
        "pagedown" => PAGE_DOWN,
        "end" => END,
        "home" => HOME,
        "left" => LEFT,
        "up" => UP,
        "right" => RIGHT,
        "down" => DOWN,
        "select" => SELECT,
        "print" => PRINT,
        "execute" => EXECUTE,
        "printscrn" => PRINT_SCRN,
        "insert" => INSERT,
        "delete" => DELETE,
        "help" => HELP,
        "tilde" | "backquote" => TILDE,
        "numlock" => NUM_LOCK,
        "greystar" => GREY_STAR,
        "greyplus" => GREY_PLUS,
        "greyminus" => GREY_MINUS,
        "greyslash" => GREY_SLASH,
        "separator" => SEPARATOR,
        "numpadperiod" => NUM_PAD_PERIOD,
        "kp_divide" => GREY_SLASH,
        "kp_multiply" => GREY_STAR,
        "mousewheelup" => MOUSE_WHEEL_UP,
        "mousewheeldown" => MOUSE_WHEEL_DOWN,
        "mousew" => MOUSE_W,
        _ => {
            if folded.len() == 1 {
                let c = folded.as_bytes()[0];
                if c.is_ascii_digit() {
                    return Some(digit(c - b'0'));
                }
                if c.is_ascii_lowercase() {
                    return Some(letter(c - b'a'));
                }
                return None;
            }
            if let Some(rest) = folded.strip_prefix("numpad") {
                return rest.parse::<u8>().ok().filter(|n| *n <= 9).map(|n| 96 + n);
            }
            let f = folded.strip_prefix('f')?;
            let f = f.parse::<u8>().ok().filter(|n| (1..=24).contains(n))?;
            function(f)
        }
    })
}

/// Inverse of [`key_from_name`] for log lines (canonical spelling).
#[allow(dead_code)]
pub fn name_for_key(value: u8) -> String {
    use key::*;
    match value {
        NONE => "None".into(),
        LEFT_MOUSE => "LeftMouse".into(),
        RIGHT_MOUSE => "RightMouse".into(),
        MIDDLE_MOUSE => "MiddleMouse".into(),
        BACKSPACE => "Backspace".into(),
        TAB => "Tab".into(),
        ENTER => "Enter".into(),
        SHIFT => "Shift".into(),
        CTRL => "Ctrl".into(),
        ALT => "Alt".into(),
        PAUSE => "Pause".into(),
        CAPS_LOCK => "CapsLock".into(),
        ESCAPE => "Escape".into(),
        SPACE => "Space".into(),
        PAGE_UP => "PageUp".into(),
        PAGE_DOWN => "PageDown".into(),
        END => "End".into(),
        HOME => "Home".into(),
        LEFT => "Left".into(),
        UP => "Up".into(),
        RIGHT => "Right".into(),
        DOWN => "Down".into(),
        INSERT => "Insert".into(),
        DELETE => "Delete".into(),
        TILDE => "Tilde".into(),
        GREY_STAR => "GreyStar".into(),
        GREY_PLUS => "GreyPlus".into(),
        GREY_MINUS => "GreyMinus".into(),
        GREY_SLASH => "GreySlash".into(),
        NUM_PAD_PERIOD => "NumPadPeriod".into(),
        MOUSE_WHEEL_UP => "MouseWheelUp".into(),
        MOUSE_WHEEL_DOWN => "MouseWheelDown".into(),
        v if (48..=57).contains(&v) => format!("{}", v - 48),
        v if (65..=90).contains(&v) => format!("{}", (b'A' + v - 65) as char),
        v if (96..=105).contains(&v) => format!("NumPad{}", v - 96),
        v if (112..=135).contains(&v) => format!("F{}", v - 111),
        other => format!("IK_{other}"),
    }
}

/// One normalized input event, mirroring `FHP2InputEvent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputEvent {
    pub key: u8,
    pub action: u8,
    pub delta: f32,
}

/// A parsed recorded-input script (`input_script_smoke.txt` grammar).
///
/// Grammar: blank lines and `#` comments ignored; first meaningful line
/// must be `tick <float>`; then `idle N`, `press K`, `hold K <N>`.
#[derive(Debug, Clone, PartialEq)]
pub struct InputScript {
    pub tick_delta: f32,
    frames: Vec<Vec<(u8, u8)>>,
}

impl InputScript {
    pub fn parse(text: &str) -> Result<Self> {
        let mut tick_delta: Option<f32> = None;
        let mut frames: Vec<Vec<(u8, u8)>> = Vec::new();
        for (line_no, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut words = line.split_whitespace();
            let op = words.next().unwrap_or_default();
            let mut arg = || {
                words.next().ok_or_else(|| {
                    EngineError::new(
                        "engine.input_script_field",
                        format!("line {}: `{line}` is missing an argument", line_no + 1),
                    )
                })
            };
            match op.to_ascii_lowercase().as_str() {
                "tick" => {
                    if tick_delta.is_some() {
                        return Err(EngineError::new(
                            "engine.input_script_tick_twice",
                            format!("line {}: second `tick` directive", line_no + 1),
                        ));
                    }
                    let value: f32 = arg()?.parse().map_err(|_| {
                        EngineError::new(
                            "engine.input_script_float",
                            format!("line {}: bad float in `{line}`", line_no + 1),
                        )
                    })?;
                    if !(value.is_finite() && value > 0.0) {
                        return Err(EngineError::new(
                            "engine.input_script_float",
                            format!(
                                "line {}: tick delta must be finite and positive",
                                line_no + 1
                            ),
                        ));
                    }
                    tick_delta = Some(value);
                }
                "idle" => {
                    let n: usize = Self::count(arg()?, line, line_no)?;
                    for _ in 0..n {
                        frames.push(Vec::new());
                    }
                }
                "press" => {
                    let key = Self::key(arg()?, line, line_no)?;
                    // IST_Press this frame, IST_Release the next.
                    frames.push(vec![(key, action::PRESS)]);
                    frames.push(vec![(key, action::RELEASE)]);
                }
                "hold" => {
                    let key = Self::key(arg()?, line, line_no)?;
                    let n: usize = Self::count(arg()?, line, line_no)?;
                    // Press, then N-1 Hold frames, then Release (N+1 total).
                    frames.push(vec![(key, action::PRESS)]);
                    for _ in 1..n {
                        frames.push(vec![(key, action::HOLD)]);
                    }
                    frames.push(vec![(key, action::RELEASE)]);
                }
                other => {
                    return Err(EngineError::new(
                        "engine.input_script_op",
                        format!("line {}: unknown directive `{other}`", line_no + 1),
                    ));
                }
            }
        }
        let Some(tick_delta) = tick_delta else {
            return Err(EngineError::new(
                "engine.input_script_no_tick",
                "script never declared a `tick <float>` delta",
            ));
        };
        Ok(Self { tick_delta, frames })
    }

    fn count(arg: &str, line: &str, line_no: usize) -> Result<usize> {
        arg.parse::<usize>().map_err(|_| {
            EngineError::new(
                "engine.input_script_count",
                format!("line {}: bad count in `{line}`", line_no + 1),
            )
        })
    }

    fn key(arg: &str, line: &str, line_no: usize) -> Result<u8> {
        key_from_name(arg).ok_or_else(|| {
            EngineError::new(
                "engine.input_script_key",
                format!("line {}: unknown key name in `{line}`", line_no + 1),
            )
        })
    }

    /// Per-frame event batches; frame i consumes `frames[i]`.
    pub fn frames(&self) -> &[Vec<(u8, u8)>] {
        &self.frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_table_contract_rows() {
        // Rows ported from Tests/InputContractTests.cpp TestInputEventMapping
        // that are platform-independent (name/value pairs).
        let rows: &[(&str, u8)] = &[
            ("a", key::letter(0)),
            ("z", key::letter(25)),
            ("9", key::digit(9)),
            ("space", key::SPACE),
            ("return", key::ENTER),
            ("kp_enter", key::ENTER),
            ("lctrl", key::CTRL),
            ("rctrl", key::CTRL),
            ("lalt", key::ALT),
            ("backquote", key::TILDE),
            ("f15", 126),
            ("kp_divide", key::GREY_SLASH),
            ("numlock", key::NUM_LOCK),
        ];
        for (name, expected) in rows {
            assert_eq!(key_from_name(name), Some(*expected), "key {name:?}");
        }
        // Ctrl collapses L/R into one key like HP2MapKeysym.
        assert_eq!(key_from_name("Ctrl"), key_from_name("LCTRL"));
        // Unmapped spellings suppress to "no key", like IK_None.
        assert_eq!(key_from_name("application"), None);
        assert_eq!(key_from_name(""), None);
    }

    #[test]
    fn press_and_hold_expansion() {
        // press K: Press this frame, Release next (2 frames).
        let script =
            InputScript::parse("tick 0.033333335\n\n# c\nidle 2\npress K\n").expect("parses");
        assert_eq!(script.tick_delta, 0.033333335);
        let f = script.frames();
        assert_eq!(f.len(), 4);
        assert_eq!(f[0], vec![]);
        assert_eq!(f[1], vec![]);
        assert_eq!(f[2], vec![(75, action::PRESS)]);
        assert_eq!(f[3], vec![(75, action::RELEASE)]);

        // hold W 40: Press, 39 Holds, Release = 41 frames.
        let script = InputScript::parse("tick 0.01\nhold W 40\n").unwrap();
        let f = script.frames();
        assert_eq!(f.len(), 41);
        assert_eq!(f[0], vec![(87, action::PRESS)]);
        assert!(f[1..40].iter().all(|fr| *fr == vec![(87, action::HOLD)]));
        assert_eq!(f[40], vec![(87, action::RELEASE)]);
    }

    #[test]
    fn script_rejects_garbage() {
        assert!(InputScript::parse("idle 3\npress A").is_err()); // no tick
        assert!(InputScript::parse("tick 1\ntick 2").is_err()); // two ticks
        assert!(InputScript::parse("tick 1\nwobble X").is_err());
        assert!(InputScript::parse("tick 1\npress NotAKey").is_err());
        assert!(InputScript::parse("tick abc").is_err());
        assert!(InputScript::parse("tick 0\nidle 1").is_err()); // non-positive
    }

    #[test]
    fn parses_smoke_fixture_shape() {
        let text = "tick 0.033333335\nidle 8\npress K\nhold Left 15\npress RightMouse\npress 1\n";
        let script = InputScript::parse(text).unwrap();
        // 8 idle + 2 + 16 + 2 + 2
        assert_eq!(script.frames().len(), 30);
    }
}
