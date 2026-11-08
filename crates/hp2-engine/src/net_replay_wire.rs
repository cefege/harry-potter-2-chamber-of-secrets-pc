//! Input-event wire-bit policy from `Engine/Src/UnReplayWire.cpp`.
//!
//! The C++ operator retains only typed `FArchive` serialization. Rust owns the
//! high-bit encoding rule, action-bit extraction, and optional-float decision.

/// Adds the analog-value flag when `delta` is nonzero.
#[inline]
pub fn encode_state(state: u8, delta: f32) -> u8 {
    state | if delta != 0.0 { 0x80 } else { 0 }
}

/// Removes the analog-value flag before publishing the input action.
#[inline]
pub fn decode_action(state: u8) -> u8 {
    state & 0x7f
}

/// Whether the wire event carries its analog value.
#[inline]
pub fn has_delta(state: u8) -> bool {
    state & 0x80 != 0
}

#[cfg(test)]
mod tests {
    use super::{decode_action, encode_state, has_delta};

    #[test]
    fn encoding_preserves_state_and_marks_nonzero_analog_values() {
        assert_eq!(encode_state(0x04, 0.0), 0x04);
        assert_eq!(encode_state(0x04, -0.0), 0x04);
        assert_eq!(encode_state(0x04, 0.25), 0x84);
        assert_eq!(encode_state(0x04, f32::NAN), 0x84);
        assert_eq!(encode_state(0xe4, 0.0), 0xe4);
    }

    #[test]
    fn decoding_masks_only_the_wire_flag() {
        assert_eq!(decode_action(0x84), 0x04);
        assert!(has_delta(0x84));
        assert!(!has_delta(0x04));
    }
}
