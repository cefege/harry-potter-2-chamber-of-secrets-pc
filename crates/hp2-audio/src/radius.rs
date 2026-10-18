//! Radius smoothing, ported from `UALAudioSubsystem::UpdateSounds`
//! (`ALAudioSubsystem.cpp` @06bfdd3 lines 1686-1702).
//!
//! The C++ shell keeps the `FTime` bookkeeping: the `WantedRadius` compare,
//! the `LastChange = CurrentTime` reset, and `elapsed = CurrentTime -
//! LastChange`. `FTime` keeps its tick count private and exposes only
//! `operator-` (a float), so the shell computes `elapsed` with that operator,
//! as the original does, and passes the float here. This module owns the lerp
//! and the choice between lerping and snapping.
//!
//! `ULevel::CalculateRadiusMultiplier` and the `OCCLUSION_FACTOR` scaling stay
//! in C++. They sit between engine calls (`IsAudibleAt`, `PointRegion`) and the
//! `ULevel` type is engine-owned.

use crate::vec3::lerp;

/// Result of one radius-smoothing step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusStep {
    /// Passed to `alSourcef(AL_REFERENCE_DISTANCE, radius * DISTANCE_FACTOR)`.
    pub radius: f32,
    /// The new `Sources(i).UsedRadius`.
    pub used_radius: f32,
}

/// `if (elapsed < 1.f) Radius = Lerp(UsedRadius, Radius, elapsed);`
/// `else UsedRadius = Radius;`
pub fn transition(used_radius: f32, radius: f32, elapsed: f32) -> RadiusStep {
    if elapsed < 1.0 {
        RadiusStep {
            radius: lerp(used_radius, radius, elapsed),
            used_radius,
        }
    } else {
        RadiusStep {
            radius,
            used_radius: radius,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Oracle: `math_oracle.cpp` in `/tmp/hp2-m2/audio/`, which includes
    // `orig_radius.inc` (ALAudioSubsystem.cpp:1690-1702 @06bfdd3) verbatim.
    // Bit patterns from `math_oracle.out`.

    #[test]
    fn transition_lerps_inside_one_second() {
        // Oracle same_mid: Radius 450, Wanted 450, Used 100, elapsed 0.25.
        let s = transition(100.0, 450.0, 0.25);
        assert_eq!(s.radius.to_bits(), 0x433b_8000);
        assert_eq!(s.used_radius.to_bits(), 0x42c8_0000);
    }

    #[test]
    fn transition_snaps_after_one_second() {
        // Oracle settled: elapsed 2.0.
        let s = transition(100.0, 450.0, 2.0);
        assert_eq!(s.radius.to_bits(), 0x43e1_0000);
        assert_eq!(s.used_radius.to_bits(), 0x43e1_0000);
    }

    #[test]
    fn transition_at_zero_elapsed_keeps_used_radius() {
        // Oracle change_0: wanted changed, so elapsed is 0 after the reset.
        let s = transition(100.0, 100.0, 0.0);
        assert_eq!(s.radius.to_bits(), 0x42c8_0000);
        assert_eq!(s.used_radius.to_bits(), 0x42c8_0000);
    }
}
