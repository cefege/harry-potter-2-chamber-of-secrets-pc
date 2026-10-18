//! Per-source fade in/out, ported from `UALAudioSubsystem::UpdateSounds`
//! (`ALAudioSubsystem.cpp` @06bfdd3 lines 1636-1665).
//!
//! The C++ shell owns the `FSource` fields and the `StopSoundSource` call.
//! This module returns what to do: the new volume, the new fade state, and
//! whether the source must be stopped.

/// `EFadeMode` (`UnAudio.h:43-45`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeMode {
    None,
    In,
    Out,
}

/// Result of one fade step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FadeStep {
    pub volume: f32,
    pub mode: FadeMode,
    pub fade_time: f32,
    /// `true` when a fade-out completed; the shell calls `StopSoundSource`
    /// and skips the rest of the per-source update (`continue`).
    pub stop: bool,
}

/// Advances one fade by `delta_time` seconds.
///
/// `FadeTime` only accumulates while `delta_time < 1.f`. `FADE_In` ends
/// the fade once the time reaches the duration; `FADE_Out` multiplies the
/// volume by `1.f - FadeTime/FadeDuration` and stops at the duration.
pub fn step(
    mode: FadeMode,
    volume: f32,
    fade_time: f32,
    fade_duration: f32,
    delta_time: f32,
) -> FadeStep {
    let mut fade_time = fade_time;
    let mut volume = volume;
    let mut mode = mode;
    let mut stop = false;

    match mode {
        FadeMode::In => {
            if delta_time < 1.0 {
                fade_time += delta_time;
            }
            if fade_time >= fade_duration {
                mode = FadeMode::None;
            } else {
                volume *= fade_time / fade_duration;
            }
        }
        FadeMode::Out => {
            if delta_time < 1.0 {
                fade_time += delta_time;
            }
            if fade_time >= fade_duration {
                stop = true;
            } else {
                volume *= 1.0 - fade_time / fade_duration;
            }
        }
        FadeMode::None => {}
    }

    FadeStep {
        volume,
        mode,
        fade_time,
        stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(s: FadeStep) -> (u32, FadeMode, u32, bool) {
        (s.volume.to_bits(), s.mode, s.fade_time.to_bits(), s.stop)
    }

    // Oracle: `/tmp/hp2-m2/audio/math_oracle.cpp` (build: `clang++ -std=c++17 -w -O0
    // -ffp-contract=off -o math_oracle math_oracle.cpp && ./math_oracle`), fragment
    // `orig_fade.inc` = `ALAudioSubsystem.cpp:1638-1665`. Expected bits from `math_oracle.out`.

    #[test]
    fn fade_in_matches_original() {
        assert_eq!(
            bits(step(FadeMode::In, 1.0, 0.0, 1.0, 0.25)),
            (0x3e800000, FadeMode::In, 0x3e800000, false)
        );
        assert_eq!(
            bits(step(FadeMode::In, 0.8, 0.5, 1.0, 0.2)),
            (0x3f0f5c29, FadeMode::In, 0x3f333333, false)
        );
    }

    #[test]
    fn fade_in_completes_and_clears_mode() {
        assert_eq!(
            bits(step(FadeMode::In, 1.0, 0.9, 1.0, 0.2)),
            (0x3f800000, FadeMode::None, 0x3f8ccccd, false)
        );
    }

    #[test]
    fn large_delta_does_not_accumulate_time() {
        assert_eq!(
            bits(step(FadeMode::In, 1.0, 0.1, 1.0, 1.5)),
            (0x3dcccccd, FadeMode::In, 0x3dcccccd, false)
        );
    }

    #[test]
    fn fade_out_matches_original() {
        assert_eq!(
            bits(step(FadeMode::Out, 1.0, 0.0, 1.0, 0.3)),
            (0x3f333333, FadeMode::Out, 0x3e99999a, false)
        );
        assert_eq!(
            bits(step(FadeMode::Out, 0.7, 0.25, 0.9, 0.1)),
            (0x3edb05b0, FadeMode::Out, 0x3eb33333, false)
        );
    }

    #[test]
    fn fade_out_stops_at_duration() {
        let s = step(FadeMode::Out, 1.0, 0.8, 1.0, 0.5);
        assert!(s.stop);
        assert_eq!(s.fade_time.to_bits(), 0x3fa66666);
        assert_eq!(s.mode, FadeMode::Out);
    }

    #[test]
    fn fade_none_is_unchanged() {
        assert_eq!(
            bits(step(FadeMode::None, 0.5, 0.0, 1.0, 0.5)),
            (0x3f000000, FadeMode::None, 0x00000000, false)
        );
    }
}
