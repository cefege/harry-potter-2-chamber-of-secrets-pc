use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, FCheckResult, ULevel, USound, UViewport};
use hp2_abi::FVector;
use hp2_audio::host::{ActorInfo, HostFailed, HostResult, ViewportInfo};
use hp2_audio::vec3::Vec3;
use hp2_core::core_access::RuntimeError;
use std::ptr;

const TRACE_VIS_BLOCKING: u32 = 0x06;

fn host_result<T>(host: &mut super::CoreHost<'_>, result: Result<T, RuntimeError>) -> HostResult<T> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            if host.error.is_none() { host.error = Some(error); }
            Err(HostFailed)
        }
    }
}

fn vec3(value: FVector) -> Vec3 {
    [value.x, value.y, value.z]
}

impl super::CoreHost<'_> {
    pub(super) fn world_actor_info(&mut self, actor: usize) -> HostResult<ActorInfo> {
        if actor == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Actor in audio host")));
        }
        let actor = actor as *const AActor;
        // SAFETY: the audio subsystem passes a live canonical actor pointer.
        let value = unsafe { &*actor };
        Ok(ActorInfo {
            location: vec3(value.location),
            velocity: vec3(value.velocity),
            sound_pitch: value.sound_pitch,
            sound_volume: value.sound_volume,
            sound_radius: f32::from(value.sound_radius),
            sound_occlusion: value.sound_occlusion,
            ambient_sound: value.ambient_sound as usize,
            index: value.index,
            // SAFETY: the actor is live; GetLevel is a plain XLevel field read.
            level: unsafe { crate::actor::get_level(actor) } as usize,
            actor_level: value.level as usize,
            light_is_none: value.light_type == 0,
            light_brightness: value.light_brightness,
        })
    }

    pub(super) fn world_actor_is_owned_by(&mut self, actor: usize, owner: usize) -> HostResult<bool> {
        // SAFETY: these are canonical actor pointers (the owner may be null).
        Ok(unsafe { crate::actor::is_owned_by(actor as *const AActor, owner as *const AActor) != 0 })
    }

    pub(super) fn world_level_is_audible_at(
        &mut self,
        level: usize,
        sound_location: Vec3,
        listener_location: Vec3,
        actor: usize,
        occlusion: u8,
    ) -> HostResult<bool> {
        if level == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Level in audio host")));
        }
        let level = level as *mut ULevel;
        let start = FVector { x: sound_location[0], y: sound_location[1], z: sound_location[2] };
        let end = FVector { x: listener_location[0], y: listener_location[1], z: listener_location[2] };
        let result = match occlusion {
            3 => {
                let mut hit = unsafe { std::mem::zeroed::<FCheckResult>() };
                // SAFETY: level and optional source actor are canonical; the trace owns its scratch links.
                unsafe {
                    crate::level_trace_owner::single_line_check(
                        self.access, level, &mut hit, actor as *mut AActor, end, start,
                        TRACE_VIS_BLOCKING, FVector { x: 0.0, y: 0.0, z: 0.0 }, 0,
                    )
                }
            }
            0 | 2 => {
                // SAFETY: level's canonical model and arrays remain live for this synchronous query.
                unsafe { crate::collision_owner::model_fast_line_check((*level).model.cast(), start, end) }
            }
            _ => Ok(true),
        };
        host_result(self, result)
    }

    pub(super) fn world_level_point_region_zone(&mut self, level: usize, actor_level: usize, location: Vec3) -> HostResult<i32> {
        if level == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Level in audio host")));
        }
        let level = level as *mut ULevel;
        // SAFETY: level, its model, and its zone actors are live canonical records.
        let result = unsafe {
            crate::collision_owner::CollisionOwner::model_point_region(
                (*level).model.cast(), actor_level as *mut _,
                FVector { x: location[0], y: location[1], z: location[2] },
            )
        };
        host_result(self, result.map(|region| i32::from(region.zone_number)))
    }

    pub(super) fn world_level_radius_multiplier(&mut self, level: usize, zone_a: i32, zone_b: i32) -> HostResult<f32> {
        if level == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Level in audio host")));
        }
        // SAFETY: level is a live canonical ULevel; zone indices retain original unchecked semantics.
        Ok(unsafe { crate::level::level_calculate_radius_multiplier(level as *mut _, zone_a, zone_b) })
    }

    pub(super) fn world_viewport_info(&mut self, viewport: usize) -> HostResult<ViewportInfo> {
        if viewport == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Viewport in audio host")));
        }
        const SHOW_REAL_TIME: i32 = 0x0000_4000;
        const SHOW_PLAYER_CTRL: i32 = 0x0000_0800;
        // SAFETY: viewport is a live canonical record; absent actor/view target is checked before dereference.
        let viewport = unsafe { &*(viewport as *const UViewport) };
        if viewport.actor.is_null() {
            return host_result(self, Err(RuntimeError::new("Canonical Viewport has no Actor")));
        }
        let pawn = unsafe { &*viewport.actor.cast::<APlayerPawn>() };
        let viewport_actor = unsafe { &*viewport.actor.cast::<AActor>() };
        let view_target = if pawn.view_target.is_null() { viewport.actor } else { pawn.view_target };
        let target = unsafe { &*view_target.cast::<AActor>() };
        if viewport_actor.level.is_null() {
            return host_result(self, Err(RuntimeError::new("Canonical Viewport Actor has no Level")));
        }
        let level = unsafe { &*viewport_actor.level.cast::<ALevelInfo>() };
        Ok(ViewportInfo {
            realtime: (pawn.show_flags & (SHOW_REAL_TIME | SHOW_PLAYER_CTRL)) != 0,
            paused: level.pauser.num > 0,
            view_target: view_target as usize,
            // SAFETY: target is a live actor and GetLevel reads its XLevel field.
            view_target_level: unsafe { crate::actor::get_level(view_target.cast()) } as usize,
            view_target_actor_level: target.level as usize,
            viewport_actor_level: viewport_actor.level as usize,
            view_target_location: vec3(target.location),
        })
    }

    pub(super) fn world_level_actor_count(&mut self, level: usize) -> HostResult<usize> {
        if level == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Level in audio host")));
        }
        // SAFETY: level is a live canonical ULevel owned by this CoreAccess.
        let count = unsafe { crate::level_owner::actor_count(level as *mut ULevel) };
        host_result(self, count.map(|count| count.max(0) as usize))
    }

    pub(super) fn world_level_actor_fill(&mut self, level: usize, out: &mut [usize]) -> HostResult<()> {
        if level == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical Level in audio host")));
        }
        let level_ptr = level as *mut ULevel;
        // SAFETY: level is a live canonical ULevel owned by this CoreAccess.
        let count = unsafe { crate::level_owner::actor_count(level_ptr) };
        let count = match count {
            Ok(count) if count >= 0 => count as usize,
            Ok(_) => return host_result(self, Err(RuntimeError::new("Negative canonical Level actor count"))),
            Err(error) => return host_result(self, Err(error)),
        };
        if count != out.len() {
            return host_result(self, Err(RuntimeError::new("Audio host Level actor count changed since count")));
        }
        // SAFETY: array storage is live and actor_at is used only for indices below its count.
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = unsafe { crate::level_owner::actor_at(level_ptr, index as i32) } as usize;
        }
        Ok(())
    }

    pub(super) fn world_lipsync_set_frame(&mut self, channel: usize, sound: usize, elapsed: f32) -> HostResult<()> {
        if channel == 0 || sound == 0 {
            return host_result(self, Err(RuntimeError::new("Null canonical lip-sync object")));
        }
        let channel = channel as *mut AActor;
        let sound = sound as *mut USound;
        // SAFETY: channel is a live canonical animation actor and sound owns initialized lip-sync data.
        unsafe {
            let lip = ptr::addr_of_mut!((*sound).lip_sync_data).cast::<crate::audio::LipSyncDataRaw>();
            let mouth_position = crate::audio::lip_mouth_pos(lip, elapsed);
            ptr::addr_of_mut!((*channel).anim_frame).write_unaligned(mouth_position);
        }
        Ok(())
    }
}
