//! Voice playback and voice selection: `PlaySound`, `StopSoundSource`,
//! `OldStopSound`, the two `StopSound` overloads, `ModifySound`, and
//! `NoteDestroy`, ported line-for-line from `ALAudioSubsystem.cpp`.
//!
//! Engine services (sound registration, actor/level queries, viewport data,
//! lip sync channels) are reached through [`Host`]. Every host call that can
//! throw in C++ returns [`HostResult`], and `?` propagates it without touching
//! the voice state that the original would not have reached.

use crate::al::{
    AL_BUFFER, AL_FALSE, AL_GAIN, AL_LOOPING, AL_PITCH, AL_POSITION, AL_REFERENCE_DISTANCE,
    AL_ROLLOFF_FACTOR, AL_SOURCE_RELATIVE, AL_TRUE, AL_VELOCITY,
};
use crate::ambient::{SF_LOOPING, SLOT_AMBIENT};
use crate::fade::FadeMode;
use crate::host::{Host, HostResult, LogKind};
use crate::lipsync::{LipSync, ftime_get_float};
use crate::listener::DISTANCE_FACTOR;
use crate::priority::{
    LeastImportantSlot, SourceSlot, find_least_important_sound,
    find_least_priority_non_talk_source, sound_priority,
};
use crate::subsystem::{
    Config, SF_HAS_LIPSYNC, SF_MUSIC, SF_NO3D, SF_STREAMING, Source, Subsystem,
};
use crate::vec3::{Vec3, clamp, dist_squared, scale};

/// `SLOT_None` (`EngineClasses.h:488-495`).
const SLOT_NONE: i32 = 0;
/// `SLOT_Talk` (`EngineClasses.h:488-495`).
const SLOT_TALK: i32 = 5;
/// `OCCLUSION_FACTOR` (`ALAudioSubsystem.h`).
pub const OCCLUSION_FACTOR: f32 = 0.35;
/// `PLAYING_PRIORITY_MULTIPLIER` (`ALAudioSubsystem.h`).
const PLAYING_PRIORITY_MULTIPLIER: f32 = 1.0;
/// `SOUND_Volume` (`EngineClasses.h:124-126`).
pub const SOUND_VOLUME: u8 = 0;
/// `SOUND_Radius` (`EngineClasses.h:124-126`).
pub const SOUND_RADIUS: u8 = 1;
/// `SOUND_Pitch` (`EngineClasses.h:124-126`).
pub const SOUND_PITCH: u8 = 2;

/// The `PlaySound` arguments, after the C++ shell has resolved the UObject
/// fields it passes in.
#[derive(Clone, Copy, Debug)]
pub struct PlayRequest {
    pub actor: usize,
    pub id: i32,
    pub sound: usize,
    pub location: Vec3,
    pub volume: f32,
    pub radius: f32,
    pub pitch: f32,
    pub flags: i32,
    pub fade_duration: f32,
    pub in_priority: f32,
}

/// The slot match shared by `StopSound(Actor, Id, Sound, ...)` and `ModifySound`.
fn matches_slot(source: &Source, actor: usize, id: i32, sound: usize) -> bool {
    let same_slot = (source.id & !1) == (id & !1) || ((id & 14) == SLOT_NONE && source.id < 0);
    source.actor == actor && same_slot && (source.sound == sound || sound == 0)
}

impl Subsystem {
    /// `UALAudioSubsystem::StopSoundSource`.
    pub fn stop_sound_source<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        index: usize,
    ) -> HostResult<()> {
        let sound = self.sources[index].sound;
        let id = self.sources[index].id;
        if sound != 0 {
            self.unregister_sound(host, sound, id)?;
        }
        Ok(())
    }

    /// `UALAudioSubsystem::OldStopSound`. Stops the AL source, detaches its
    /// buffer, releases lip sync, and clears the identity fields. `Radius`,
    /// `Volume`, and the other fields are left untouched, as in the original.
    pub fn old_stop_sound<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        index: usize,
    ) -> HostResult<()> {
        if self.sources[index].id == 0 {
            return Ok(());
        }
        let source = self.sources[index].source;
        if source != 0 {
            host.al_source_stop(source);
            host.al_source_i(source, AL_BUFFER, 0);
        }
        if let Some(lip_sync) = self.sources[index].lip_sync.take() {
            lip_sync.release(host)?;
        }
        let s = &mut self.sources[index];
        s.sound = 0;
        s.actor = 0;
        s.flags = 0;
        s.priority = 0.0;
        s.id = 0;
        s.started = false;
        s.paused = false;
        Ok(())
    }

    /// `UALAudioSubsystem::PlaySound`. Returns the voice index, or -1 when no
    /// voice is available.
    ///
    /// A null `req.sound` returns -1 here. The original dereferences it and
    /// crashes; that is a deliberate behavior change.
    pub fn play_sound<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        req: &PlayRequest,
    ) -> HostResult<i32> {
        if req.sound == 0 {
            return Ok(-1);
        }
        let info = host.sound_info(req.sound)?;
        let name = host.sound_name(req.sound)?;
        let note = if (info.core_flags & SF_STREAMING) != 0 {
            "Playing Streaming sound "
        } else {
            "Playing sound "
        };
        host.log(LogKind::DevSound, &format!("{}{}", note, name))?;
        host.sound_set_working_flags(req.sound, info.core_flags | req.flags)?;

        let is_talk = (req.id & 14) == SLOT_TALK * 2;
        if self.viewport == 0 {
            return Ok(-1);
        }

        if !self.register_sound(host, req.sound)? {
            let index = if is_talk {
                let slots: Vec<SourceSlot> = self
                    .sources
                    .iter()
                    .map(|s| SourceSlot {
                        has_sound: s.sound != 0,
                        id: s.id,
                        priority: s.priority,
                    })
                    .collect();
                find_least_priority_non_talk_source(&slots)
            } else {
                self.find_least_important(host, cfg, req.sound)?
            };
            if index == -1 {
                return Ok(-1);
            }
            self.stop_sound_source(host, index as usize)?;
            if !self.register_sound(host, req.sound)? {
                return Ok(-1);
            }
        }

        let mut volume = clamp(req.volume * cfg.sound_volume, 0.0, 1.0);

        // Priority, using the occlusion-scaled radius when the sound is occluded.
        let priority = if is_talk {
            100.0
        } else if req.in_priority != 0.0 && (req.id & 14) == SLOT_AMBIENT * 2 {
            req.in_priority
        } else {
            let mut occlusion_radius = req.radius;
            if let Some(occluded) =
                self.occluded_radius(host, req.actor, req.location, req.radius, req.flags)?
            {
                occlusion_radius = occluded * OCCLUSION_FACTOR;
            }
            self.sound_priority_at(host, cfg, req.location, volume, occlusion_radius, req.flags)?
        };

        let mut best_priority = priority;
        let mut id = req.id;
        if (req.id & 14) == 2 * SLOT_NONE {
            self.free_slot -= 1;
            id = 16 * self.free_slot;
        }

        let mut index: i32 = -1;
        for (i, source) in self.sources.iter().enumerate() {
            if (source.id & !1) == (id & !1) {
                if (id & 1) != 0 {
                    return Ok(-1);
                }
                index = i as i32;
                break;
            } else if source.priority * PLAYING_PRIORITY_MULTIPLIER < best_priority {
                index = i as i32;
                best_priority = source.priority;
            }
        }
        if index == -1 {
            index = self.find_least_important(host, cfg, req.sound)?;
            if index == -1 {
                return Ok(-1);
            }
        }
        let index = index as usize;
        self.stop_sound_source(host, index)?;

        if (info.core_flags & SF_STREAMING) != 0 {
            host.sound_bind_stream(req.sound, id)?;
        }

        let pitch = clamp(req.pitch, 0.1, 2.0);
        volume = clamp(volume, 0.01, 1.0);

        // Radius defaults, then the occlusion adjustment for 3D sounds.
        let mut zone_radius = req.radius;
        let mut used_radius = req.radius;
        let mut wanted_radius = req.radius;
        if let Some(occluded) =
            self.occluded_radius(host, req.actor, req.location, req.radius, req.flags)?
        {
            zone_radius = occluded;
            used_radius = occluded * OCCLUSION_FACTOR;
            wanted_radius = used_radius;
        }

        let working = host.sound_info(req.sound)?.working_flags;
        let location = if (req.flags & SF_NO3D) != 0 {
            [0.0, 0.0, 0.0]
        } else {
            req.location
        };
        let volume_field = if (req.flags & SF_MUSIC) != 0 {
            cfg.music_volume
        } else {
            volume
        };
        let fade_in = req.fade_duration > 0.0;
        let last_change = host.app_seconds()?;

        let flags = if (working & SF_STREAMING) != 0 {
            let handle = host.sound_get_handle(req.sound, id)?;
            req.flags | self.streams[(handle - 1) as usize].flags
        } else {
            let handle = host.sound_get_handle(req.sound, -1)?;
            req.flags | self.buffers[(handle - 1) as usize].flags
        };

        let lip_sync = if (working & SF_HAS_LIPSYNC) != 0 && req.actor != 0 {
            Some(LipSync::new(req.actor, req.sound))
        } else {
            None
        };

        {
            let s = &mut self.sources[index];
            s.sound = req.sound;
            s.id = id;
            s.actor = req.actor;
            s.priority = priority;
            s.location = location;
            s.radius = if req.radius != 0.0 { req.radius } else { 10.0 };
            s.zone_radius = zone_radius;
            s.used_radius = used_radius;
            s.wanted_radius = wanted_radius;
            s.last_change = last_change;
            s.fade_duration = req.fade_duration;
            s.fade_time = 0.0;
            s.fade_mode = if fade_in {
                FadeMode::In
            } else {
                FadeMode::None
            };
            s.volume = volume_field;
            s.started = true;
            s.paused = false;
            s.flags = flags;
            if let Some(ls) = lip_sync {
                s.lip_sync = Some(ls);
            }
        }

        let sid = self.sources[index].source;
        host.al_source_f(sid, AL_REFERENCE_DISTANCE, used_radius * DISTANCE_FACTOR);
        host.al_source_f(sid, AL_GAIN, if fade_in { 0.0 } else { volume_field });
        host.al_source_f(sid, AL_PITCH, pitch);
        let looping = if (working & SF_STREAMING) != 0 {
            AL_FALSE
        } else if (req.flags & SF_LOOPING) != 0 {
            AL_TRUE
        } else {
            AL_FALSE
        };
        host.al_source_i(sid, AL_LOOPING, looping);
        host.al_source_fv(sid, AL_POSITION, scale(location, DISTANCE_FACTOR));
        host.al_source_fv(sid, AL_VELOCITY, [0.0, 0.0, 0.0]);
        host.al_source_f(sid, AL_ROLLOFF_FACTOR, cfg.roll_off);
        host.al_source_i(
            sid,
            AL_SOURCE_RELATIVE,
            if (req.flags & SF_NO3D) != 0 {
                AL_TRUE
            } else {
                AL_FALSE
            },
        );

        if (working & SF_STREAMING) != 0 {
            let handle = host.sound_get_handle(req.sound, id)?;
            let stream = &self.streams[(handle - 1) as usize];
            let count = stream.st.num_buffers;
            let buffers = stream.st.buffers;
            host.al_source_queue_buffers(sid, &buffers[..count]);
        } else {
            let handle = host.sound_get_handle(req.sound, -1)?;
            let buffer = self.buffers[(handle - 1) as usize].buffer;
            host.al_source_i(sid, AL_BUFFER, buffer as i32);
        }

        host.al_source_stop(sid);
        host.al_source_rewind(sid);
        host.al_source_play(sid);

        if self.sources[index].lip_sync.is_some() {
            let now = ftime_get_float(host.app_seconds()?);
            if let Some(ls) = self.sources[index].lip_sync.as_mut() {
                ls.start(host, now)?;
            }
        }
        Ok(index as i32)
    }

    /// `UALAudioSubsystem::SoundPriority` for an arbitrary location, volume, and radius.
    pub(crate) fn sound_priority_at<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        location: Vec3,
        volume: f32,
        radius: f32,
        flags: i32,
    ) -> HostResult<f32> {
        let mut dist_sq = 0.0;
        if radius != 0.0 {
            let view = host.viewport_info(self.viewport)?;
            dist_sq = dist_squared(location, view.view_target_location);
        }
        Ok(sound_priority(
            dist_sq,
            radius,
            cfg.max_radius_multiplier,
            volume,
            flags,
        ))
    }

    /// The `!IsAudibleAt` branch shared by both occlusion blocks in PlaySound.
    /// Returns `Some(radius * CalculateRadiusMultiplier(...))` when the sound is
    /// occluded, and `None` when it is audible or the actor has no level.
    fn occluded_radius<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        actor: usize,
        location: Vec3,
        radius: f32,
        flags: i32,
    ) -> HostResult<Option<f32>> {
        if (flags & SF_NO3D) != 0 || actor == 0 {
            return Ok(None);
        }
        let info = host.actor_info(actor)?;
        if info.level == 0 {
            return Ok(None);
        }
        if host.level_is_audible_at(
            info.level,
            location,
            self.last_position,
            actor,
            info.sound_occlusion,
        )? {
            return Ok(None);
        }
        let zone = host.level_point_region_zone(info.level, info.actor_level, location)?;
        let multiplier =
            host.level_radius_multiplier(info.level, zone, self.region_listener_zone)?;
        Ok(Some(radius * multiplier))
    }

    /// `UALAudioSubsystem::FindLeastImportantSound` over the live voices.
    fn find_least_important<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        cfg: &Config,
        incoming: usize,
    ) -> HostResult<i32> {
        let mut view_target: Option<Vec3> = None;
        let mut slots = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            let mut dist_sq = 0.0;
            if source.radius != 0.0 {
                let target = match view_target {
                    Some(target) => target,
                    None => {
                        let target = host.viewport_info(self.viewport)?.view_target_location;
                        view_target = Some(target);
                        target
                    }
                };
                dist_sq = dist_squared(source.location, target);
            }
            let flags = if source.sound != 0 {
                host.sound_info(source.sound)?.working_flags
            } else {
                0
            };
            slots.push(LeastImportantSlot {
                sound: source.sound,
                id: source.id,
                dist_squared: dist_sq,
                volume: source.volume,
                radius: source.radius,
                flags,
            });
        }
        Ok(find_least_important_sound(
            &slots,
            incoming,
            cfg.max_radius_multiplier,
        ))
    }

    /// `UALAudioSubsystem::StopSound(AActor*, USound*)`. Returns whether any voice stopped.
    pub fn stop_sound_by_actor<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        actor: usize,
        sound: usize,
    ) -> HostResult<bool> {
        if actor == 0 && sound == 0 {
            for i in 0..self.sources.len() {
                self.stop_sound_source(host, i)?;
            }
            return Ok(true);
        }
        let mut stopped = false;
        for i in 0..self.sources.len() {
            let s = &self.sources[i];
            let hit = if sound != 0 {
                if actor != 0 {
                    actor == s.actor && sound == s.sound
                } else {
                    sound == s.sound
                }
            } else {
                actor == s.actor
            };
            if hit {
                self.stop_sound_source(host, i)?;
                stopped = true;
            }
        }
        Ok(stopped)
    }

    /// `UALAudioSubsystem::StopSound(AActor*, INT Id, USound*, FLOAT fFadeOutTime)`.
    /// A positive fade time starts a fade-out. Otherwise the voice is stopped.
    pub fn stop_sound_by_id<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        actor: usize,
        id: i32,
        sound: usize,
        fade_out_time: f32,
    ) -> HostResult<bool> {
        let mut stopped = false;
        for i in 0..self.sources.len() {
            if !matches_slot(&self.sources[i], actor, id, sound) {
                continue;
            }
            if fade_out_time > 0.0 {
                let s = &mut self.sources[i];
                s.fade_time = 0.0;
                s.fade_duration = fade_out_time;
                s.fade_mode = FadeMode::Out;
            } else {
                self.stop_sound_source(host, i)?;
            }
            stopped = true;
        }
        Ok(stopped)
    }

    /// `UALAudioSubsystem::ModifySound`. Returns true when any slot matches.
    /// Unknown parameter codes match and return true, but change nothing,
    /// as in the original switch with no default.
    pub fn modify_sound<H: Host + ?Sized>(
        &mut self,
        host: &mut H,
        actor: usize,
        id: i32,
        sound: usize,
        parameter: u8,
        value: f32,
    ) -> HostResult<bool> {
        let mut ret = false;
        for i in 0..self.sources.len() {
            if !matches_slot(&self.sources[i], actor, id, sound) {
                continue;
            }
            let sid = self.sources[i].source;
            match parameter {
                SOUND_VOLUME => {
                    let volume = clamp(value, 0.01, 1.0);
                    self.sources[i].volume = volume;
                    host.al_source_f(sid, AL_GAIN, volume);
                }
                SOUND_RADIUS => {
                    {
                        let s = &mut self.sources[i];
                        s.zone_radius = value;
                        s.used_radius = value;
                        s.wanted_radius = value;
                    }
                    let src_actor = self.sources[i].actor;
                    if (self.sources[i].flags & SF_NO3D) == 0 && src_actor != 0 {
                        let info = host.actor_info(src_actor)?;
                        if info.level != 0
                            && !host.level_is_audible_at(
                                info.level,
                                info.location,
                                self.last_position,
                                src_actor,
                                info.sound_occlusion,
                            )?
                        {
                            let zone = host.level_point_region_zone(
                                info.level,
                                info.actor_level,
                                info.location,
                            )?;
                            let temp = value
                                * host.level_radius_multiplier(
                                    info.level,
                                    zone,
                                    self.region_listener_zone,
                                )?;
                            self.sources[i].zone_radius = temp;
                            let temp = temp * OCCLUSION_FACTOR;
                            self.sources[i].used_radius = temp;
                            self.sources[i].wanted_radius = temp;
                        }
                    }
                    host.al_source_f(
                        sid,
                        AL_REFERENCE_DISTANCE,
                        self.sources[i].used_radius * DISTANCE_FACTOR,
                    );
                }
                SOUND_PITCH => {
                    let pitch = clamp(value, 0.1, 2.0);
                    host.al_source_f(sid, AL_PITCH, pitch);
                }
                _ => {}
            }
            ret = true;
        }
        Ok(ret)
    }

    /// `UALAudioSubsystem::NoteDestroy`. Ambient voices for a destroyed actor
    /// are stopped. Other voices just lose their actor pointer.
    pub fn note_destroy<H: Host + ?Sized>(&mut self, host: &mut H, actor: usize) -> HostResult<()> {
        for i in 0..self.sources.len() {
            if self.sources[i].actor != actor {
                continue;
            }
            if (self.sources[i].id & 14) == SLOT_AMBIENT * 2 {
                self.stop_sound_source(host, i)?;
            } else {
                self.sources[i].actor = 0;
            }
        }
        Ok(())
    }
}
