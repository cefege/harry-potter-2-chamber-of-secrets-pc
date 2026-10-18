//! `UALAudioSubsystem::Exec` command bodies.
//!
//! The C++ shell keeps the engine's `ParseCommand` dispatch, the `ASTAT` /
//! `WEAPONRADIUS` / `ROLLOFF` handling (`GAudioDefaultRadius` is an engine global and
//! `RollOff` is a UObject config property), and the `Ar` output device. Rust owns the
//! command bodies for PAUSESOUNDS, UNPAUSESOUNDS, RESTARTSOUNDS, and PLAYINGSOUNDSTATS.

use crate::host::{Host, HostResult};

use crate::subsystem::{OGGVORBIS_STREAM_CHUNKSIZE, SF_STREAMING, Subsystem, XA_STREAM_CHUNKSIZE};
use crate::update::stream_index;

/// Exec commands whose bodies live in Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecCommand {
    PauseSounds,
    UnpauseSounds,
    RestartSounds,
    PlayingSoundStats,
}

impl Subsystem {
    /// Runs one Exec command body. The caller returns `1` to the engine after `Ok`.
    pub fn exec<H: Host + ?Sized>(&mut self, host: &mut H, cmd: ExecCommand) -> HostResult<()> {
        match cmd {
            ExecCommand::PauseSounds => {
                self.pause_sounds(host);
                Ok(())
            }
            ExecCommand::UnpauseSounds => {
                self.unpause_sounds(host);
                Ok(())
            }
            ExecCommand::RestartSounds => {
                self.restart_sounds = true;
                Ok(())
            }
            ExecCommand::PlayingSoundStats => self.playing_sound_stats(host),
        }
    }

    /// `PLAYINGSOUNDSTATS`. `iOverhead` and `iTotalMem` are deliberately NOT reset per
    /// source, matching the original accumulation. The loop is bounded by the
    /// live `sources` count. The original runs `i < MAX_AUDIOSTREAMS` over
    /// `Sources(i)`, which reads past `Sources.Num()` when
    /// `[ALAudio.ALAudioSubsystem] Channels` is below 32; that read is undefined
    /// behavior in the original and is not reproduced. Identical at `Channels=32`.
    fn playing_sound_stats<H: Host + ?Sized>(&mut self, host: &mut H) -> HostResult<()> {
        let mut total_mem: i32 = 0;
        let mut overhead: i32 = 0;

        for i in 0..self.sources.len() {
            let sound = self.sources[i].sound;
            if sound == 0 {
                continue;
            }
            let info = host.sound_info(sound)?;
            let data_size = host.sound_data_len(sound)?;
            let name = host.sound_name(sound)?;
            let al_buffer_size: i32;

            if (info.working_flags & SF_STREAMING) != 0 {
                let id = self.sources[i].id;
                let handle = host.sound_get_handle(sound, id)?;
                let stream = &self.streams[stream_index(handle)];
                let streaming_buffer_size = if info.is_ogg {
                    OGGVORBIS_STREAM_CHUNKSIZE
                } else {
                    XA_STREAM_CHUNKSIZE
                };
                overhead += streaming_buffer_size;

                let mut sum: i32 = 0;
                for j in 0..stream.st.num_buffers {
                    sum += host.al_buffer_size(stream.st.buffers[j]);
                }
                al_buffer_size = sum;

                host.output(&format!(
                    "Streaming Sound:  Name = {}  Sound data = {}K  ALAudio buffer = {}K  Streaming buffer = {}K",
                    name,
                    data_size / 1024,
                    al_buffer_size / 1024,
                    streaming_buffer_size / 1024
                ))?;
            } else {
                let handle = host.sound_get_handle(sound, -1)?;
                let index = usize::try_from(handle - 1).expect("non-streaming sound has a buffer");
                let buffer = self.buffers[index].buffer;
                al_buffer_size = host.al_buffer_size(buffer);
                host.output(&format!(
                    "Sound:  Name = {}  Sound data = {}K  ALAudio buffer = {}K",
                    name,
                    data_size / 1024,
                    al_buffer_size / 1024
                ))?;
            }

            overhead += al_buffer_size;
            total_mem += overhead + data_size;
        }

        host.output(&format!(
            "Total memory = {}K  Overhead = {}K",
            total_mem / 1024,
            overhead / 1024
        ))?;
        Ok(())
    }
}
