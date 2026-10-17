//! Replaces the joystick event mapping inline in
//! `USDLViewport::UpdateInput`'s `SDL_JOYAXISMOTION`/
//! `SDL_JOYBUTTONDOWN`/`SDL_JOYBUTTONUP`/`SDL_JOYHATMOTION` cases
//! (`SDLViewport.cpp:373-415`). Unlike `HP2MapKeysym` and friends,
//! this logic has no existing named C++ function of its own -- it is
//! written directly inline in the switch statement, not behind a
//! header the way `HP2InputEvents.h`/`HP2MouseCapturePolicy.h` are.
//! So there is no existing oracle test and no existing abstraction
//! boundary to preserve; this module is new structure, not a forward
//! for something that already had one.

use crate::input_event::{ik, ist};

/// `MaxJoystickAxes` (`SDLDrv.h:31`), matching `joystick.rs`'s own
/// copy of the same constant (kept separate rather than shared,
/// since the two modules are otherwise independent and this avoids a
/// cross-module dependency for one `i32`).
const MAX_JOYSTICK_AXES: i32 = 8;
/// `GJoyAxisKeys` (`SDLViewport.cpp:28-31`): joystick axes in the
/// exact order the engine's input contracts expect.
const JOY_AXIS_KEYS: [i32; MAX_JOYSTICK_AXES as usize] = [
    ik::JOY_X,
    ik::JOY_Y,
    ik::JOY_Z,
    ik::JOY_R,
    ik::JOY_U,
    ik::JOY_V,
    ik::UNKNOWN_EA,
    ik::UNKNOWN_EB,
];
/// The engine's `IK_Joy1..IK_Joy16` range, matching
/// `joystick.rs::MAX_JOYSTICK_BUTTONS`.
const MAX_JOYSTICK_BUTTONS: i32 = 16;
/// `GHatButtonBase` (`SDLViewport.cpp:34`): hats fold onto buttons
/// starting at `IK_Joy13` once twelve real buttons exist.
const HAT_BUTTON_BASE: i32 = 12;

/// Replaces the `SDL_JOYAXISMOTION` case (`SDLViewport.cpp:373-386`).
/// `axis` is `Event.jaxis.axis`, `raw_value` is `Event.jaxis.value`
/// (an `Sint16`, passed widened to `i32`). Returns `None` when `axis`
/// is out of range (matching the real C++'s own
/// `if (Event.jaxis.axis >= MaxJoystickAxes) break;` guard), else
/// `Some((key, scaled_value))` for the caller's own
/// `CauseInputEvent(key, IST_Axis, scaled_value)` call.
pub fn map_joystick_axis(
    axis: i32,
    raw_value: i32,
    dead_zone: f32,
    scale_x: f32,
    scale_y: f32,
) -> Option<(i32, f32)> {
    if !(0..MAX_JOYSTICK_AXES).contains(&axis) {
        return None;
    }
    let mut value = raw_value as f32 / 32768.0;
    if value > -dead_zone && value < dead_zone {
        value = 0.0;
    }
    let scale = match axis {
        0 => scale_x,
        1 => scale_y,
        _ => 1.0,
    };
    if scale != 0.0 {
        value *= scale;
    }
    Some((JOY_AXIS_KEYS[axis as usize], value))
}

/// Replaces the `SDL_JOYBUTTONDOWN`/`SDL_JOYBUTTONUP` case
/// (`SDLViewport.cpp:388-395`). `button` is `Event.jbutton.button`,
/// `pressed` is `Event.type == SDL_JOYBUTTONDOWN`. Returns `None` when
/// `button >= 16` (matching the real C++'s own guard), else
/// `Some((key, action))`.
pub fn map_joystick_button(button: i32, pressed: bool) -> Option<(i32, i32)> {
    if !(0..MAX_JOYSTICK_BUTTONS).contains(&button) {
        return None;
    }
    let action = if pressed { ist::PRESS } else { ist::RELEASE };
    Some((ik::JOY1 + button, action))
}

/// Replaces the `SDL_JOYHATMOTION` case (`SDLViewport.cpp:396-415`).
/// `hat_bits` is `Event.jhat.value` (an `SDL_HAT_*` bitmask),
/// `last_folded` is the real C++'s own `LastJoyHat` field, read
/// before the call. Returns `(new_last_folded, events)`, where
/// `events` holds 0, 1, or 2 `(key, action)` pairs for the caller to
/// run through `CauseInputEvent` in order, and `new_last_folded` is
/// what the caller stores back into its own `LastJoyHat` field
/// afterward (Rust does not own it). The real C++'s own
/// `if (!JoyHatFoldsToButtons) break;` guard happens at the call
/// site, not here: this function only runs the fold/transition math.
pub fn fold_joystick_hat(hat_bits: u8, last_folded: i32) -> (i32, [(i32, i32); 2], usize) {
    const SDL_HAT_UP: u8 = 0x01;
    const SDL_HAT_RIGHT: u8 = 0x02;
    const SDL_HAT_DOWN: u8 = 0x04;
    const SDL_HAT_LEFT: u8 = 0x08;

    let folded = if hat_bits & SDL_HAT_UP != 0 {
        ik::JOY1 + HAT_BUTTON_BASE
    } else if hat_bits & SDL_HAT_DOWN != 0 {
        ik::JOY1 + HAT_BUTTON_BASE + 1
    } else if hat_bits & SDL_HAT_LEFT != 0 {
        ik::JOY1 + HAT_BUTTON_BASE + 2
    } else if hat_bits & SDL_HAT_RIGHT != 0 {
        ik::JOY1 + HAT_BUTTON_BASE + 3
    } else {
        ik::NONE
    };

    if folded == last_folded {
        return (last_folded, [(0, 0); 2], 0);
    }

    let mut events = [(0, 0); 2];
    let mut count = 0;
    if last_folded != ik::NONE {
        events[count] = (last_folded, ist::RELEASE);
        count += 1;
    }
    if folded != ik::NONE {
        events[count] = (folded, ist::PRESS);
        count += 1;
    }
    (folded, events, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `SDLViewport.cpp:373-386`.
    #[test]
    fn map_joystick_axis_matches_the_real_cpp() {
        // Out of range.
        assert_eq!(map_joystick_axis(8, 32767, 0.2, 1.0, 1.0), None);
        assert_eq!(map_joystick_axis(-1, 32767, 0.2, 1.0, 1.0), None);

        // Full-scale positive, no scale (axis 2, uses the 1.0 default).
        let (key, value) = map_joystick_axis(2, 32767, 0.2, 1.0, 1.0).unwrap();
        assert_eq!(key, ik::JOY_Z);
        assert!((value - 32767.0 / 32768.0).abs() < 1e-6);

        // Dead zone: a small value clamps to exactly 0.
        let (key, value) = map_joystick_axis(0, 1000, 0.2, 1.0, 1.0).unwrap();
        assert_eq!(key, ik::JOY_X);
        assert_eq!(value, 0.0);

        // Axis 0 uses scale_x; axis 1 uses scale_y; a zero scale is a
        // no-op multiply (matching the real C++'s `if (Scale != 0.f)`
        // guard, which leaves Value unscaled rather than zeroing it).
        let (key, value) = map_joystick_axis(0, 16384, 0.0, 2.0, 1.0).unwrap();
        assert_eq!(key, ik::JOY_X);
        assert!((value - (16384.0 / 32768.0) * 2.0).abs() < 1e-6);

        let (key, value) = map_joystick_axis(1, 16384, 0.0, 1.0, 0.0).unwrap();
        assert_eq!(key, ik::JOY_Y);
        assert!(
            (value - 16384.0 / 32768.0).abs() < 1e-6,
            "zero scale must not zero the value"
        );

        // Axis 7 (last valid index) maps to UNKNOWN_EB, matching the
        // real GJoyAxisKeys table's own last entry.
        let (key, _) = map_joystick_axis(7, 0, 0.2, 1.0, 1.0).unwrap();
        assert_eq!(key, ik::UNKNOWN_EB);
    }

    /// Mirrors `SDLViewport.cpp:388-395`.
    #[test]
    fn map_joystick_button_matches_the_real_cpp() {
        assert_eq!(map_joystick_button(0, true), Some((ik::JOY1, ist::PRESS)));
        assert_eq!(
            map_joystick_button(15, false),
            Some((ik::JOY1 + 15, ist::RELEASE))
        );
        assert_eq!(
            map_joystick_button(16, true),
            None,
            "button 16 is out of the IK_Joy1..16 range"
        );
        assert_eq!(map_joystick_button(-1, true), None);
    }

    /// Mirrors `SDLViewport.cpp:396-415`.
    #[test]
    fn fold_joystick_hat_matches_the_real_cpp_transitions() {
        // First press (from IK_None): one PRESS event, no RELEASE.
        let (last, events, count) = fold_joystick_hat(0x01 /* UP */, ik::NONE);
        assert_eq!(last, ik::JOY1 + 12);
        assert_eq!(count, 1);
        assert_eq!(events[0], (ik::JOY1 + 12, ist::PRESS));

        // Same direction held: no transition, no events.
        let (last, _events, count) = fold_joystick_hat(0x01, ik::JOY1 + 12);
        assert_eq!(last, ik::JOY1 + 12);
        assert_eq!(count, 0);

        // Direction change: RELEASE old, PRESS new, in that order.
        let (last, events, count) = fold_joystick_hat(0x04 /* DOWN */, ik::JOY1 + 12);
        assert_eq!(last, ik::JOY1 + 13);
        assert_eq!(count, 2);
        assert_eq!(events[0], (ik::JOY1 + 12, ist::RELEASE));
        assert_eq!(events[1], (ik::JOY1 + 13, ist::PRESS));

        // Centered (no direction bits set): RELEASE only, folds back
        // to IK_None.
        let (last, events, count) = fold_joystick_hat(0x00, ik::JOY1 + 13);
        assert_eq!(last, ik::NONE);
        assert_eq!(count, 1);
        assert_eq!(events[0], (ik::JOY1 + 13, ist::RELEASE));

        // Centered while already centered: no transition.
        let (last, _events, count) = fold_joystick_hat(0x00, ik::NONE);
        assert_eq!(last, ik::NONE);
        assert_eq!(count, 0);

        // UP takes priority over RIGHT when both bits are set
        // (diagonal), matching the real C++'s if/else-if chain order
        // (UP, DOWN, LEFT, RIGHT).
        let (last, _, _) = fold_joystick_hat(0x01 | 0x02 /* UP | RIGHT */, ik::NONE);
        assert_eq!(
            last,
            ik::JOY1 + 12,
            "UP must win over RIGHT in the if/else-if chain"
        );
    }
}
