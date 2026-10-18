//! Music entry points ported from `ALAudioSubsystem.cpp:1161-1268`:
//! `PlayMusic`, `StopMusic`, `StopAllMusic`, `PauseSounds`, `UnpauseSounds`.
//!
//! Music is a streamed voice in the normal source table. The USound object is
//! created by the C++ shell through `Host::music_sound_new`, which applies the
//! original `new USound(name, SF_Streaming | SF_Music)` and `FileType = "ogg"`.

use crate::fade::FadeMode;
use crate::host::{Host, HostResult, LogKind};
use crate::playback::PlayRequest;
use crate::subsystem::{Config, SF_MUSIC, SF_NO3D, SF_STREAMING, Subsystem};

/// `appFExt` (`UnMisc.cpp:293-308`). The result is the text after the first
/// `:`, the last `/`, and the last `.`, in that order. With no `.`, it is the
/// basename, not an empty string.
pub fn app_fext(name: &str) -> &str {
    let mut s = name;
    if let Some(p) = s.find(':') {
        s = &s[p + 1..];
    }
    if let Some(p) = s.rfind('/') {
        s = &s[p + 1..];
    }
    if let Some(p) = s.rfind('.') {
        s = &s[p + 1..];
    }
    s
}

impl Subsystem {
    /// `UALAudioSubsystem::PlayMusic`. Returns the one-based song handle, or 0.
    pub fn play_music<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        song: &str,
        fade_in_time: f32,
    ) -> HostResult<i32> {
        if song.is_empty() || song.eq_ignore_ascii_case("none") {
            return Ok(0);
        }

        let append_ext = !app_fext(song).eq_ignore_ascii_case("ogg");
        let mut filename = String::from("..\\music\\");
        filename.push_str(song);
        if append_ext {
            filename.push_str(".ogg");
        }

        host.log(
            LogKind::DevMusic,
            &format!("PlayMusic request: {} -> {}", song, filename),
        )?;
        let music = host.music_sound_new(&filename)?;
        if music == 0 {
            host.log(
                LogKind::Warning,
                &format!("PlayMusic could not allocate {}", filename),
            )?;
            return Ok(0);
        }

        // PlaySound(NULL, 2*SLOT_None | 1, Music, FVector(0,0,0), MusicVolume,
        //           1000, 1.f, SF_Streaming | SF_No3D | SF_Music, FadeInTime)
        let request = PlayRequest {
            actor: 0,
            id: 1,
            sound: music,
            location: [0.0, 0.0, 0.0],
            volume: cfg.music_volume,
            radius: 1000.0,
            pitch: 1.0,
            flags: SF_STREAMING | SF_NO3D | SF_MUSIC,
            fade_duration: fade_in_time,
            in_priority: 0.0,
        };
        let source_index = self.play_sound(host, cfg, &request)?;
        if source_index < 0 {
            host.log(
                LogKind::Warning,
                &format!("PlayMusic could not start {}", filename),
            )?;
            return Ok(0);
        }
        host.log(
            LogKind::DevMusic,
            &format!("PlayMusic started source {}: {}", source_index, filename),
        )?;
        Ok(source_index + 1)
    }

    /// `UALAudioSubsystem::StopMusic`. `song_handle` is the one-based value
    /// `PlayMusic` returned.
    pub fn stop_music<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        song_handle: i32,
        fade_out_time: f32,
    ) -> HostResult<bool> {
        let index = song_handle.wrapping_sub(1);
        if index < 0 || (index as usize) >= self.sources.len() {
            return Ok(false);
        }
        let index = index as usize;
        if fade_out_time > 0.0 {
            let source = &mut self.sources[index];
            source.fade_time = 0.0;
            source.fade_duration = fade_out_time;
            source.fade_mode = FadeMode::Out;
        } else {
            self.stop_sound_source(host, index)?;
        }
        Ok(true)
    }

    /// `UALAudioSubsystem::StopAllMusic`. Always returns 0 in the original, so
    /// it returns nothing here.
    pub fn stop_all_music<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        fade_out_time: f32,
    ) -> HostResult<()> {
        for i in 0..self.sources.len() {
            if (self.sources[i].flags & SF_MUSIC) != 0 {
                self.stop_music(host, i as i32 + 1, fade_out_time)?;
            }
        }
        Ok(())
    }

    /// `UALAudioSubsystem::PauseSounds`.
    pub fn pause_sounds<H: Host + ?Sized>(&mut self, host: &mut H) {
        for source in self.sources.iter_mut() {
            if source.id != 0 {
                source.paused = true;
                host.al_source_pause(source.source);
            }
        }
    }

    /// `UALAudioSubsystem::UnpauseSounds`.
    pub fn unpause_sounds<H: Host + ?Sized>(&mut self, host: &mut H) {
        for source in self.sources.iter_mut() {
            if source.id != 0 {
                source.paused = false;
                if source.started {
                    host.al_source_play(source.source);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::app_fext;

    // Oracle: `/tmp/hp2-m2/audio/fext/fext_oracle.cpp`, built with
    // `clang++ -std=c++17 -w -O0 -ffp-contract=off`. The original
    // `appFExt` body is included verbatim from 06bfdd3 `UnMisc.cpp:293-308`.
    // Output: `fext_oracle.out`.
    #[test]
    fn matches_cpp_app_fext() {
        assert_eq!(app_fext("Sounds/Music/song.ogg"), "ogg");
        assert_eq!(app_fext("ogg:Music/a.b.ogg"), "ogg");
        assert_eq!(app_fext("noext"), "noext");
        assert_eq!(app_fext("dir.x/file"), "file");
        assert_eq!(app_fext("a.b.c.ogg"), "ogg");
        assert_eq!(app_fext("C:\\x.wav"), "wav");
        assert_eq!(app_fext(""), "");
        assert_eq!(app_fext("/"), "");
        assert_eq!(app_fext("."), "");
        assert_eq!(app_fext("x:y/z.w:v"), "w:v");
        assert_eq!(app_fext("trailing."), "");
        assert_eq!(app_fext("a/b/c"), "c");
    }
}
