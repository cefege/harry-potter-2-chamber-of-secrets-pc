//! `FLipSyncAnimMgr` and the `FTime` conversion it uses, ported from
//! `ALAudioSubsystem.cpp:2175-2260` and `Core.h:29-64`.
//!
//! The animation channel, `CreateAnimChannel`, `PlayAnim`, and the mouth-frame
//! lookup on `FLipSyncData` are engine objects. They stay in the C++ shell and
//! are reached through `Host::lipsync_*`. This module owns only the timing
//! state.
//!
//! Release is explicit: `release` consumes the value and returns a `HostResult`.
//! A `Drop` impl would call into the engine during unwinding and could throw.

use crate::host::{Host, HostResult};

/// `FIXTIME` (`Core.h:29`). `FTime` stores 64-bit ticks at this scale.
pub const FIXTIME: f32 = 4294967296.0;

/// `LIP_SYNC_UPDATE_INTERVAL` (`ALAudioSubsystem.cpp`).
const LIP_SYNC_UPDATE_INTERVAL: f32 = 0.05;

/// `FTime::GetFloat`: `(FLOAT)Value / FIXTIME`. `ticks` is the raw `FTime`
/// tick count, which the C++ shell reads as `int64_t`.
#[inline]
pub fn ftime_get_float(ticks: i64) -> f32 {
    (ticks as f32) / FIXTIME
}

/// Per-voice lip sync state (`FLipSyncAnimMgr`).
///
/// `chan` is the host's animation-channel handle. `0` means no channel was
/// created, and `update` is then a no-op, as in the original.
#[derive(Debug)]
pub struct LipSync {
    /// `m_ParentActor`. The actor is a host handle.
    pub parent_actor: usize,
    /// `m_pLipSyncData` is `&Sound->LipSyncData`, so the sound is the handle.
    pub sound: usize,
    /// `m_StartTime`, in seconds.
    pub start_time: f32,
    /// `m_LastElapsedTime`, in seconds.
    pub last_elapsed_time: f32,
    /// `m_LipSyncAnimChan`, as a host handle. `0` is null.
    pub chan: usize,
}

impl LipSync {
    /// `FLipSyncAnimMgr(AActor*, FLipSyncData*)`.
    pub fn new(parent_actor: usize, sound: usize) -> Self {
        Self {
            parent_actor,
            sound,
            start_time: 0.0,
            last_elapsed_time: 0.0,
            chan: 0,
        }
    }

    /// `FLipSyncAnimMgr::Start(FLOAT)`. Returns whether a channel was created.
    /// The host call creates the channel and starts `mouth_anim`.
    pub fn start<H: Host + ?Sized>(&mut self, host: &mut H, now: f32) -> HostResult<bool> {
        self.start_time = now;
        self.last_elapsed_time = 0.0;
        self.chan = host.lipsync_start(self.parent_actor)?;
        Ok(self.chan != 0)
    }

    /// `FLipSyncAnimMgr::Update(FLOAT)`. Writes the mouth frame at most once per
    /// `LIP_SYNC_UPDATE_INTERVAL`.
    pub fn update<H: Host + ?Sized>(&mut self, host: &mut H, now: f32) -> HostResult<()> {
        if self.chan == 0 {
            return Ok(());
        }
        let elapsed = now - self.start_time;
        let frame_delta = elapsed - self.last_elapsed_time;
        if frame_delta >= LIP_SYNC_UPDATE_INTERVAL {
            self.last_elapsed_time = elapsed;
            host.lipsync_set_frame(self.chan, self.sound, elapsed)?;
        }
        Ok(())
    }

    /// `FLipSyncAnimMgr::EatPauseTime(FLOAT)`. Shifts the start time while the
    /// parent sound is paused.
    pub fn eat_pause_time(&mut self, delta: f32) {
        self.start_time += delta;
    }

    /// `~FLipSyncAnimMgr`. Plays `mouth_rest` on a live channel. The C++ shell
    /// implements this in `Host::lipsync_release`.
    pub fn release<H: Host + ?Sized>(self, host: &mut H) -> HostResult<()> {
        if self.chan != 0 {
            host.lipsync_release(self.chan)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ftime_get_float;

    // Oracle: `/tmp/hp2-m2/audio/fext/ftime_oracle.cpp`, built with
    // `clang++ -std=c++17 -w -O0 -ffp-contract=off`. `FTime` is included
    // verbatim from 06bfdd3 `Core.h:29-66`. Output: `ftime_oracle.out`.
    #[test]
    fn ftime_get_float_matches_cpp_bits() {
        let cases: [(i64, u32); 13] = [
            (0, 0x00000000),
            (1, 0x2f800000),
            (-1, 0xaf800000),
            (4294967296, 0x3f800000),
            (-4294967296, 0xbf800000),
            (3, 0x30400000),
            ((1 << 40) + 12345, 0x43800000),
            (9007199254740993, 0x4a000000),
            (i64::MAX, 0x4f000000),
            (i64::MIN, 0xcf000000),
            (2147483647, 0x3f000000),
            (-2147483648, 0xbf000000),
            (1 << 62, 0x4e800000),
        ];
        for (ticks, want) in cases {
            assert_eq!(ftime_get_float(ticks).to_bits(), want, "ticks {ticks}");
        }
    }
}
