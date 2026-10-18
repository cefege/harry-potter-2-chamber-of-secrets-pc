//! `UALAudioSubsystem::Update` control flow (`ALAudioSubsystem.cpp` original
//! 1331-1762): viewport and region prologue, pause/unpause transitions,
//! finished-source handling, ambient start and update, the per-source sound
//! loop (stream tick, lip sync, location/pitch/volume, fade, occlusion,
//! radius smoothing, priority, position), and the listener.
//!
//! The C++ shell only supplies the view location and axes; the host table
//! supplies every engine read and write.

use std::cell::RefCell;

use crate::al::{
    AL_BUFFERS_PROCESSED, AL_GAIN, AL_ORIENTATION, AL_PITCH, AL_POSITION, AL_REFERENCE_DISTANCE,
    AL_SOURCE_STATE, AL_STOPPED, AL_VELOCITY, al_error,
};
use crate::ambient::{
    SF_LOOPING, SF_UPDATE_PITCH, SLOT_AMBIENT, ambient_admitted, ambient_flags,
    ambient_out_of_range, ambient_source_id, sort_by_priority,
};
use crate::fade;
use crate::host::{Host, HostFailed, HostResult, LogKind, ViewportInfo};
use crate::lipsync::{FIXTIME, ftime_get_float};
use crate::listener::{
    DISTANCE_FACTOR, delta_time, listener_velocity, listener_velocity_ok, orientation,
    source_velocity_ok,
};
use crate::playback::{OCCLUSION_FACTOR, PlayRequest};
use crate::radius;
use crate::stream::{self, TickError};
use crate::subsystem::{Config, SF_NO_UPDATES, SF_NO3D, SF_STREAMING, Subsystem};
use crate::vec3::{Vec3, dist_squared, scale};

/// The view inputs `Update` receives from the C++ shell.
#[derive(Clone, Copy, Debug)]
pub struct UpdateView {
    /// `SceneNode->Coords.Origin`.
    pub view_location: Vec3,
    /// `SceneNode->Coords.XAxis`.
    pub x_axis: Vec3,
    /// `SceneNode->Coords.YAxis`.
    pub y_axis: Vec3,
    /// `SceneNode->Coords.ZAxis`.
    pub z_axis: Vec3,
}

/// A pending ambient start, collected before any `PlaySound` call.
#[derive(Clone, Copy, Debug)]
struct AmbientCandidate {
    actor: usize,
    id: i32,
    sound: usize,
    location: Vec3,
    volume: f32,
    radius: f32,
    pitch: f32,
    flags: i32,
    priority: f32,
}

/// `FTime` subtraction: the int64 tick difference, then a float divide.
fn ftime_diff(a: i64, b: i64) -> f32 {
    (a.wrapping_sub(b) as f32) / FIXTIME
}

/// Stream handles are one-based; a streaming source always holds a valid one.
pub(crate) fn stream_index(handle: i32) -> usize {
    usize::try_from(handle - 1).expect("streaming source has a valid stream handle")
}

impl Subsystem {
    /// `UALAudioSubsystem::Update`.
    pub fn update<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        view: &UpdateView,
    ) -> HostResult<()> {
        if self.viewport == 0 {
            return Ok(());
        }
        let view_location = view.view_location;
        let ds3d = cfg.use_eax && !cfg.use_mmsystem;
        let ori = orientation(
            view.x_axis,
            view.y_axis,
            view.z_axis,
            ds3d,
            cfg.reverse_stereo,
        );

        let vp: ViewportInfo = host.viewport_info(self.viewport)?;
        self.region_listener_zone = host.level_point_region_zone(
            vp.view_target_level,
            vp.view_target_actor_level,
            view_location,
        )?;

        let current_time = host.app_seconds()?;
        let delta = delta_time(
            ftime_get_float(current_time),
            ftime_get_float(self.last_time),
        );
        self.last_time = current_time;

        let realtime = vp.realtime;
        let paused = vp.paused;
        if (!realtime && self.last_realtime) || (paused && !self.last_paused) {
            self.pause_sounds(host);
        } else if (realtime && !self.last_realtime) || (!paused && self.last_paused) {
            self.unpause_sounds(host);
        } else if self.restart_sounds {
            self.restart_sounds = false;
            self.unpause_sounds(host);
        }
        self.last_realtime = realtime;
        self.last_paused = paused;

        self.handle_finished_sources(host)?;

        if realtime && !paused {
            self.handle_ambience(host, cfg, &vp, view_location)?;
        }
        self.update_ambience(host, cfg, view_location)?;
        self.update_sounds(host, cfg, &vp, view_location, current_time, delta)?;

        // Listener (original 1730-1749). The velocity uses the unscaled
        // view location; the position is scaled afterwards.
        let orient: [f32; 6] = [
            ori.up[0],
            ori.up[1],
            ori.up[2],
            ori.front[0],
            ori.front[1],
            ori.front[2],
        ];
        let velocity = listener_velocity(view_location, self.last_position, delta);
        self.last_position = view_location;
        let position = scale(view_location, DISTANCE_FACTOR);
        host.al_listener_fv(AL_POSITION, &position);
        host.al_listener_fv(AL_ORIENTATION, &orient);
        if listener_velocity_ok(velocity) {
            host.al_listener_fv(AL_VELOCITY, &velocity);
        }
        Ok(())
    }

    /// "Check for finished sounds" (original 1395-1430).
    fn handle_finished_sources<H: Host + ?Sized>(&mut self, host: &mut H) -> HostResult<()> {
        for i in 0..self.sources.len() {
            let (sound, id, source, flags) = {
                let s = &self.sources[i];
                (s.sound, s.id, s.source, s.flags)
            };
            if id == 0 || sound == 0 || (flags & SF_LOOPING) != 0 {
                continue;
            }
            if host.al_get_source_i(source, AL_SOURCE_STATE) != AL_STOPPED {
                continue;
            }
            if (flags & SF_STREAMING) != 0 {
                let idx = stream_index(host.sound_get_handle(sound, id)?);
                if self.streams[idx].st.alive {
                    // Stream ran out of buffers.
                    let name = host.sound_name(sound)?;
                    let now = host.app_seconds()?;
                    let time = (ftime_get_float(now) * 10000.0) as i32;
                    host.log(
                        LogKind::DevSound,
                        &format!("WARNING: audio buffer underrun - {name}   time = {time}"),
                    )?;
                    self.streams[idx].st.reset = true;
                } else {
                    let name = host.sound_name(sound)?;
                    host.log(
                        LogKind::DevSound,
                        &format!(
                            "WARNING: was check(0) from Vogel, now UnregisterSound({}, {}) - {}",
                            sound as i32, id, name
                        ),
                    )?;
                    self.unregister_sound(host, sound, id)?;
                }
            } else {
                self.stop_sound_source(host, i)?;
            }
        }
        Ok(())
    }

    /// `HandleAmbience` (original 1440-1500), run only when realtime and not paused.
    fn handle_ambience<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        vp: &ViewportInfo,
        view_location: Vec3,
    ) -> HostResult<()> {
        let count = host.level_actor_count(vp.view_target_level)?;
        let mut actors = vec![0usize; count];
        host.level_actor_fill(vp.view_target_level, &mut actors)?;

        let mut candidates: Vec<AmbientCandidate> = Vec::new();
        for actor in actors {
            if actor == 0 {
                continue;
            }
            let info = host.actor_info(actor)?;
            if info.ambient_sound == 0 {
                continue;
            }
            let id = ambient_source_id(info.index);
            if self.sources.iter().any(|s| s.id == id) {
                continue;
            }
            let volume = info.sound_volume as f32 / 255.0;
            let pitch = info.sound_pitch as f32 / 64.0;
            if host.actor_is_owned_by(actor, vp.view_target)? {
                let priority = self.sound_priority_at(
                    host,
                    cfg,
                    info.location,
                    volume,
                    info.sound_radius,
                    SF_LOOPING | SF_NO3D | SF_UPDATE_PITCH,
                )?;
                candidates.push(AmbientCandidate {
                    actor,
                    id,
                    sound: info.ambient_sound,
                    location: info.location,
                    volume,
                    radius: info.sound_radius,
                    pitch,
                    flags: ambient_flags(true),
                    priority,
                });
            } else {
                let mut radius_multiplier = 1.0f32;
                let audible = host.level_is_audible_at(
                    vp.view_target_level,
                    info.location,
                    view_location,
                    actor,
                    info.sound_occlusion,
                )?;
                if !audible {
                    let zone = host.level_point_region_zone(
                        vp.view_target_level,
                        info.actor_level,
                        info.location,
                    )?;
                    radius_multiplier *= host.level_radius_multiplier(
                        vp.view_target_level,
                        zone,
                        self.region_listener_zone,
                    )?;
                }
                let dist = dist_squared(view_location, info.location);
                if ambient_admitted(
                    false,
                    dist,
                    info.sound_radius,
                    radius_multiplier,
                    cfg.max_radius_multiplier,
                ) {
                    let priority = self.sound_priority_at(
                        host,
                        cfg,
                        info.location,
                        volume,
                        radius_multiplier * info.sound_radius,
                        SF_LOOPING | SF_UPDATE_PITCH,
                    )?;
                    candidates.push(AmbientCandidate {
                        actor,
                        id,
                        sound: info.ambient_sound,
                        location: info.location,
                        volume,
                        radius: info.sound_radius,
                        pitch,
                        flags: ambient_flags(false),
                        priority,
                    });
                }
            }
        }

        if !candidates.is_empty() {
            sort_by_priority(&mut candidates, |c| c.priority);
            for c in &candidates {
                let req = PlayRequest {
                    actor: c.actor,
                    id: c.id,
                    sound: c.sound,
                    location: c.location,
                    volume: c.volume,
                    radius: c.radius,
                    pitch: c.pitch,
                    flags: c.flags,
                    fade_duration: 0.0,
                    in_priority: c.priority,
                };
                if self.play_sound(host, cfg, &req)? == -1 {
                    break;
                }
            }
        }
        Ok(())
    }

    /// `UpdateAmbience` (original 1500-1525). Runs every update, not only when realtime.
    fn update_ambience<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        view_location: Vec3,
    ) -> HostResult<()> {
        for i in 0..self.sources.len() {
            let (id, actor, sound, zone_radius) = {
                let s = &self.sources[i];
                (s.id, s.actor, s.sound, s.zone_radius)
            };
            if (id & 14) != SLOT_AMBIENT * 2 || actor == 0 {
                continue;
            }
            let info = host.actor_info(actor)?;
            let new_dist = dist_squared(view_location, info.location);
            if ambient_out_of_range(new_dist, zone_radius, cfg.max_radius_multiplier)
                || info.ambient_sound != sound
            {
                self.stop_sound_source(host, i)?;
            }
        }
        Ok(())
    }

    /// The per-source loop of `UpdateSounds` (original 1527-1762).
    fn update_sounds<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        vp: &ViewportInfo,
        view_location: Vec3,
        current_time: i64,
        delta: f32,
    ) -> HostResult<()> {
        for i in 0..self.sources.len() {
            let (id, paused, flags, source) = {
                let s = &self.sources[i];
                (s.id, s.paused, s.flags, s.source)
            };
            if id == 0 {
                continue;
            }
            if paused {
                if let Some(ls) = self.sources[i].lip_sync.as_mut() {
                    ls.eat_pause_time(delta);
                }
                continue;
            }

            if (flags & SF_STREAMING) != 0 {
                self.update_streaming(host, i)?;
            }

            if let Some(ls) = self.sources[i].lip_sync.as_mut() {
                ls.update(host, ftime_get_float(current_time))?;
            }

            // UpdateActorLocPitchVolAndFadeInFadeOut (original 1613-1645).
            let actor = self.sources[i].actor;
            let mut velocity: Vec3 = [0.0; 3];
            let mut volume = self.sources[i].volume;
            if actor != 0 && (flags & SF_NO_UPDATES) == 0 {
                let info = host.actor_info(actor)?;
                self.sources[i].location = info.location;
                if (flags & SF_NO3D) == 0 {
                    velocity = scale(info.velocity, DISTANCE_FACTOR);
                }
                if (flags & SF_UPDATE_PITCH) != 0 {
                    host.al_source_f(source, AL_PITCH, info.sound_pitch as f32 / 64.0);
                }
                if (id & 14) == SLOT_AMBIENT * 2 {
                    volume = info.sound_volume as f32 / 255.0;
                    self.sources[i].volume = volume;
                }
                if !info.light_is_none {
                    volume *= info.light_brightness as f32 / 255.0;
                }
            }

            // Fade (original 1636-1645).
            let fade_step = fade::step(
                self.sources[i].fade_mode,
                volume,
                self.sources[i].fade_time,
                self.sources[i].fade_duration,
                delta,
            );
            self.sources[i].fade_mode = fade_step.mode;
            self.sources[i].fade_time = fade_step.fade_time;
            if fade_step.stop {
                self.stop_sound_source(host, i)?;
                continue;
            }
            volume = fade_step.volume;

            host.al_source_f(source, AL_GAIN, volume);

            if (flags & SF_NO3D) == 0 {
                self.update_reference_distance(host, vp, i, view_location, current_time)?;
            }

            let location = self.sources[i].location;
            let priority_volume = self.sources[i].volume;
            let zone_radius = self.sources[i].zone_radius;
            let priority =
                self.sound_priority_at(host, cfg, location, priority_volume, zone_radius, flags)?;
            self.sources[i].priority = priority;

            if (flags & SF_NO3D) == 0 {
                let loc = scale(self.sources[i].location, DISTANCE_FACTOR);
                // The original scales the velocity a second time here. Keep it.
                velocity = scale(velocity, DISTANCE_FACTOR);
                host.al_source_fv(source, AL_POSITION, loc);
                if source_velocity_ok(velocity) {
                    host.al_source_fv(source, AL_VELOCITY, velocity);
                }
            }
        }
        Ok(())
    }

    /// Streaming half of the per-source loop (original 1538-1600).
    fn update_streaming<H: Host + ?Sized>(&mut self, host: &mut H, i: usize) -> HostResult<()> {
        let (sound, id, source) = {
            let s = &self.sources[i];
            (s.sound, s.id, s.source)
        };
        let idx = stream_index(host.sound_get_handle(sound, id)?);
        let processed = host.al_get_source_i(source, AL_BUFFERS_PROCESSED);
        al_error(host, "Before queueing.", false)?;

        let mut work = self.streams[idx].st.clone();
        let (format, chunk, rate, stream_id) = {
            let s = &self.streams[idx];
            (s.format, s.stream_chunk_size, s.rate, s.id)
        };
        let buffers = work.buffers;
        let refill = work.refill;

        let result = {
            let cell = RefCell::new(&mut *host);
            stream::tick(
                &mut work,
                processed,
                || {
                    let mut h = cell.borrow_mut();
                    let buffer = h.al_source_unqueue_buffer(source);
                    al_error(&mut **h, "unqueueing", true)?;
                    Ok(buffer)
                },
                || {
                    let mut h = cell.borrow_mut();
                    h.gfile_pop_completed_chunk(stream_id - 1)
                },
                |index| {
                    let mut h = cell.borrow_mut();
                    h.al_buffer_data_at(buffers[index], format, refill[index], chunk, rate);
                    al_error(&mut **h, "bufferdata", true)?;
                    h.al_source_queue_buffers(source, &[buffers[index]]);
                    al_error(&mut **h, "queueing", true)?;
                    Ok(())
                },
                |dest| {
                    let mut h = cell.borrow_mut();
                    h.gfile_request_chunk(stream_id - 1, dest)
                },
            )
        };

        match result {
            Ok(outcome) => {
                self.streams[idx].st = work;
                if outcome.played {
                    host.al_source_play(source);
                }
                Ok(())
            }
            Err(TickError::Stream(e)) => {
                host.app_error(&format!("stream state check failed: {e:?}"))?;
                Err(HostFailed)
            }
            Err(TickError::Host(_)) => Err(HostFailed),
        }
    }

    /// Occlusion, zone radius, and radius smoothing for a 3D source, then
    /// `AL_REFERENCE_DISTANCE` (original 1648-1703).
    fn update_reference_distance<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        vp: &ViewportInfo,
        i: usize,
        view_location: Vec3,
        current_time: i64,
    ) -> HostResult<()> {
        let (source, actor, location) = {
            let s = &self.sources[i];
            (s.source, s.actor, s.location)
        };
        let occlusion = if actor != 0 {
            host.actor_info(actor)?.sound_occlusion
        } else {
            0
        };
        let mut radius = self.sources[i].radius;
        let audible = host.level_is_audible_at(
            vp.view_target_level,
            location,
            view_location,
            actor,
            occlusion,
        )?;
        if !audible {
            let region_source = host.level_point_region_zone(
                vp.view_target_level,
                vp.viewport_actor_level,
                location,
            )?;
            radius *= host.level_radius_multiplier(
                vp.view_target_level,
                region_source,
                self.region_listener_zone,
            )?;
            self.sources[i].zone_radius = radius;
            radius *= OCCLUSION_FACTOR;
        } else {
            self.sources[i].zone_radius = radius;
        }

        if self.sources[i].wanted_radius != radius {
            self.sources[i].last_change = current_time;
            self.sources[i].wanted_radius = radius;
        }
        let elapsed = ftime_diff(current_time, self.sources[i].last_change);
        let step = radius::transition(self.sources[i].used_radius, radius, elapsed);
        self.sources[i].used_radius = step.used_radius;
        host.al_source_f(source, AL_REFERENCE_DISTANCE, step.radius * DISTANCE_FACTOR);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ftime_diff;

    // Oracle: `/tmp/hp2-m2/audio/fext/ftime_oracle.cpp` (`operator-`, 06bfdd3
    // `Core.h:29-66`, verbatim), built with
    // `clang++ -std=c++17 -w -O0 -ffp-contract=off`. Output: `ftime_oracle.out`.
    #[test]
    fn ftime_diff_matches_cpp_operator_minus_bits() {
        let cases: [(i64, i64, u32); 8] = [
            (0, 0, 0x00000000),
            (5, 3, 0x30000000),
            (3, 5, 0xb0000000),
            (4294967296, 0, 0x3f800000),
            (1 << 40, 1 << 39, 0x43000000),
            (i64::MAX, 0, 0x4f000000),
            (i64::MIN, 0, 0xcf000000),
            (0, 1, 0xaf800000),
        ];
        for (a, b, want) in cases {
            assert_eq!(ftime_diff(a, b).to_bits(), want, "diff {a} {b}");
        }
    }
}
