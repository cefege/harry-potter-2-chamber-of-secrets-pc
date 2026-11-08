//! AActor sound propagation and audio command policy; Unreal objects stay in C++.
use std::ffi::c_void;
use std::ptr::null_mut;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hp2ActorAudioVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2ActorAudioSound {
    pub sound: *mut c_void,
    pub slot: u8,
    pub volume: f32,
    pub no_override: i32,
    pub radius: f32,
    pub pitch: f32,
    pub disable_3d: i32,
    pub looping: i32,
}

// These selectors expose single engine fields/queries, never routing policy.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hp2ActorAudioObject {
    AudioLevel,
    AudioDemoDriver,
    AudioServerConnection,
    AudioClient,
    AudioViewTarget,
    AudioInstigator,
    AudioOwner,
    AudioPlayer,
    AudioPawnList,
    AudioNextPawn,
    AudioSubsystem,
}
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hp2ActorAudioInteger {
    AudioNetMode,
    AudioActorIndex,
    AudioIsPlayerPawn,
    AudioIsViewport,
    AudioIsPlayer,
    AudioViewportCount,
    AudioCallerSimulated,
}
#[repr(C)]
pub struct Hp2ActorAudioServices {
    pub context: *mut c_void,
    pub object: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        Hp2ActorAudioObject,
        *mut *mut c_void,
    ) -> i32,
    pub integer:
        unsafe extern "C" fn(*mut c_void, *mut c_void, Hp2ActorAudioInteger, *mut i32) -> i32,
    pub location: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut Hp2ActorAudioVector) -> i32,
    pub viewport_actor:
        unsafe extern "C" fn(*mut c_void, *mut c_void, i32, *mut *mut c_void) -> i32,
    pub line_check: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *const Hp2ActorAudioVector,
        *const Hp2ActorAudioVector,
        *mut i32,
    ) -> i32,
    pub hear: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        i32,
        *mut c_void,
        *const Hp2ActorAudioVector,
        *const Hp2ActorAudioVector,
        i32,
        i32,
    ) -> i32,
    pub demo: unsafe extern "C" fn(*mut c_void, *mut c_void, *const Hp2ActorAudioSound) -> i32,
    pub play_music:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, f32, *mut i32) -> i32,
    pub stop_music: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, f32) -> i32,
    pub stop_all_music: unsafe extern "C" fn(*mut c_void, *mut c_void, f32) -> i32,
    pub modify_sound: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *mut c_void,
        i32,
        *mut c_void,
        u8,
        f32,
        *mut i32,
    ) -> i32,
    pub stop_sound:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, i32, *mut c_void, f32) -> i32,
}
use Hp2ActorAudioInteger::*;
use Hp2ActorAudioObject::*;

fn status(code: i32) -> Result<(), i32> {
    if code == 0 { Ok(()) } else { Err(code) }
}
impl Hp2ActorAudioServices {
    unsafe fn object(
        &self,
        object: *mut c_void,
        field: Hp2ActorAudioObject,
    ) -> Result<*mut c_void, i32> {
        let mut out = null_mut();
        status(unsafe { (self.object)(self.context, object, field, &mut out) })?;
        Ok(out)
    }
    unsafe fn integer(&self, object: *mut c_void, field: Hp2ActorAudioInteger) -> Result<i32, i32> {
        let mut out = 0;
        status(unsafe { (self.integer)(self.context, object, field, &mut out) })?;
        Ok(out)
    }
    unsafe fn location(&self, actor: *mut c_void) -> Result<Hp2ActorAudioVector, i32> {
        let mut out = Hp2ActorAudioVector::default();
        status(unsafe { (self.location)(self.context, actor, &mut out) })?;
        Ok(out)
    }
}

pub fn sound_id(index: i32, slot: u8, no_override: i32) -> i32 {
    index
        .wrapping_mul(16)
        .wrapping_add(i32::from(slot) * 2)
        .wrapping_add(no_override)
}

/// # Safety
/// Actors and service callbacks must remain valid for the duration of the call.
pub unsafe fn check_hear_sound(
    actor: *mut c_void,
    hearer: *mut c_void,
    id: i32,
    sound: *mut c_void,
    mut parameters: Hp2ActorAudioVector,
    radius_squared: f32,
    disable_3d: i32,
    looping: i32,
    s: &Hp2ActorAudioServices,
) -> Result<(), i32> {
    unsafe {
        let mut source = hearer;
        if s.integer(hearer, AudioIsPlayerPawn)? != 0 {
            let target = s.object(hearer, AudioViewTarget)?;
            if !target.is_null() {
                source = target;
            }
        }
        let hear_source = s.location(source)?;
        let location = s.location(actor)?;
        let dx = hear_source.x - location.x;
        let dy = hear_source.y - location.y;
        let dz = hear_source.z - location.z;
        // `(HearSource-Location).SizeSquared()`, via the shared helper that documents
        // and owns clang's `fma(Z, Z, fma(X, X, Y*Y))` contraction for this exact
        // expression (`crate::vec::size_squared`, matching `hp2_core::math::dot3`).
        let distance = crate::vec::size_squared(hp2_abi::FVector {
            x: dx,
            y: dy,
            z: dz,
        });
        let mut radius = radius_squared / 1.3_f32;
        // Deliberately strict (also rejects NaN), unlike the occluded cutoff.
        if distance < radius {
            let level = s.object(actor, AudioLevel)?;
            let mut visible = 0;
            status((s.line_check)(
                s.context,
                level,
                &hear_source,
                &location,
                &mut visible,
            ))?;
            if visible == 0 {
                if s.object(actor, AudioInstigator)? != hearer {
                    radius *= 0.6_f32;
                }
                parameters.x *= 0.35_f32;
                if distance > radius {
                    return Ok(());
                }
            }
            // Re-read the source location at the event boundary, as the original does.
            let location = s.location(actor)?;
            status((s.hear)(
                s.context,
                hearer,
                actor,
                id,
                sound,
                &location,
                &parameters,
                disable_3d,
                looping,
            ))?;
        }
        Ok(())
    }
}

/// # Safety
/// `actor`, caller frame, sound arguments, and callbacks are live engine objects.
/// kind: 0 = DemoPlaySound, 1 = PlaySound, 2 = PlayOwnedSound.
pub unsafe fn play_sound(
    actor: *mut c_void,
    caller: *mut c_void,
    kind: u32,
    args: &Hp2ActorAudioSound,
    s: &Hp2ActorAudioServices,
) -> Result<(), i32> {
    unsafe {
        if args.sound.is_null() {
            return Ok(());
        }
        if kind != 0 {
            let level = s.object(actor, AudioLevel)?;
            if !level.is_null() {
                let driver = s.object(level, AudioDemoDriver)?;
                if !driver.is_null()
                    && s.object(driver, AudioServerConnection)?.is_null()
                    && (kind == 2 || s.integer(level, AudioNetMode)? != 3)
                {
                    status((s.demo)(s.context, actor, args))?;
                }
            }
        }
        // Demo event may mutate actor/level state; all propagation reads follow it.
        let id = sound_id(
            s.integer(actor, AudioActorIndex)?,
            args.slot,
            args.no_override,
        );
        let radius = if args.radius != 0.0 {
            args.radius
        } else {
            1600.0
        };
        let parameters = Hp2ActorAudioVector {
            x: 100.0 * args.volume,
            y: args.radius,
            z: 100.0 * args.pitch,
        };
        let simulated = kind == 1 && s.integer(caller, AudioCallerSimulated)? != 0;
        let level = s.object(actor, AudioLevel)?;
        let local = kind == 0 || s.integer(level, AudioNetMode)? == 3 || simulated;
        if local {
            let client = s.object(level, AudioClient)?;
            if !client.is_null() {
                let mut i = 0;
                while i < s.integer(client, AudioViewportCount)? {
                    let mut hearer = null_mut();
                    status((s.viewport_actor)(s.context, client, i, &mut hearer))?;
                    if !hearer.is_null()
                        && s.object(hearer, AudioLevel)? == s.object(actor, AudioLevel)?
                    {
                        check_hear_sound(
                            actor,
                            hearer,
                            id,
                            args.sound,
                            parameters,
                            radius * radius,
                            args.disable_3d,
                            args.looping,
                            s,
                        )?;
                    }
                    i += 1;
                }
            }
        } else {
            let mut remote_owner = null_mut();
            if kind == 2 && s.integer(level, AudioNetMode)? != 0 {
                let candidate = if s.integer(actor, AudioIsPlayerPawn)? != 0 {
                    actor
                } else {
                    let owner = s.object(actor, AudioOwner)?;
                    if !owner.is_null() && s.integer(owner, AudioIsPlayerPawn)? != 0 {
                        owner
                    } else {
                        null_mut()
                    }
                };
                if !candidate.is_null() {
                    let player = s.object(candidate, AudioPlayer)?;
                    if !player.is_null() && s.integer(player, AudioIsViewport)? == 0 {
                        remote_owner = candidate;
                    }
                }
            }
            let mut hearer = s.object(actor, AudioPawnList)?;
            while !hearer.is_null() {
                if s.integer(hearer, AudioIsPlayer)? != 0 && (kind != 2 || hearer != remote_owner) {
                    check_hear_sound(
                        actor,
                        hearer,
                        id,
                        args.sound,
                        parameters,
                        radius * radius,
                        args.disable_3d,
                        args.looping,
                        s,
                    )?;
                }
                hearer = s.object(hearer, AudioNextPawn)?;
            }
        }
        Ok(())
    }
}

/// # Safety
/// Engine pointers and result storage must be valid. `song` is an FString pointer.
/// command: 0 play music, 1 stop music, 2 stop all music, 3 modify sound, 4 stop sound.
pub unsafe fn audio_command(
    actor: *mut c_void,
    command: u32,
    object: *mut c_void,
    slot: u8,
    parameter: u8,
    handle: i32,
    value: f32,
    out: *mut i32,
    s: &Hp2ActorAudioServices,
) -> Result<(), i32> {
    unsafe {
        // StopSound computes its ID even when there is no audio subsystem.
        let stop_id = if command == 4 {
            sound_id(s.integer(actor, AudioActorIndex)?, slot, 0)
        } else {
            0
        };
        let level = s.object(actor, AudioLevel)?;
        let audio = s.object(level, AudioSubsystem)?;
        match command {
            0 => {
                let mut result = 0;
                if !audio.is_null() {
                    status((s.play_music)(s.context, audio, object, value, &mut result))?;
                }
                *out = result;
            }
            1 if !audio.is_null() => status((s.stop_music)(s.context, audio, handle, value))?,
            2 if !audio.is_null() => status((s.stop_all_music)(s.context, audio, value))?,
            3 => {
                if audio.is_null() {
                    *out = 0;
                } else {
                    let id = sound_id(s.integer(actor, AudioActorIndex)?, slot, 0);
                    status((s.modify_sound)(
                        s.context, audio, actor, id, object, parameter, value, out,
                    ))?;
                }
            }
            4 if !audio.is_null() => status((s.stop_sound)(
                s.context, audio, actor, stop_id, object, value,
            ))?,
            _ => {}
        }
        Ok(())
    }
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's `GActorAudioServices` table.
-----------------------------------------------------------------------------*/

static mut AUDIO_SERVICES: Option<Hp2ActorAudioServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorAudioServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_audio_services(value: Hp2ActorAudioServices) {
    unsafe { (&raw mut AUDIO_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_audio_services` has run.
pub unsafe fn published_audio_services() -> Result<Hp2ActorAudioServices, i32> {
    unsafe { (&raw const AUDIO_SERVICES).read().ok_or(1) }
}

#[cfg(test)]
mod check_hear_sound_tests {
    use super::*;

    const HEARER: *mut c_void = 0x1000usize as *mut c_void;
    const ACTOR: *mut c_void = 0x2000usize as *mut c_void;

    struct Context {
        hearer_loc: Hp2ActorAudioVector,
        actor_loc: Hp2ActorAudioVector,
        line_check_called: bool,
        hear_called: bool,
    }

    unsafe extern "C" fn object(
        _ctx: *mut c_void,
        _obj: *mut c_void,
        _field: Hp2ActorAudioObject,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe { out.write(0x3000usize as *mut c_void) };
        0
    }

    unsafe extern "C" fn integer(
        _ctx: *mut c_void,
        _obj: *mut c_void,
        _field: Hp2ActorAudioInteger,
        out: *mut i32,
    ) -> i32 {
        // Never a player pawn: `source` stays `hearer`.
        unsafe { out.write(0) };
        0
    }

    unsafe extern "C" fn location(
        ctx: *mut c_void,
        obj: *mut c_void,
        out: *mut Hp2ActorAudioVector,
    ) -> i32 {
        let c = unsafe { &*ctx.cast::<Context>() };
        let v = if obj == HEARER {
            c.hearer_loc
        } else {
            c.actor_loc
        };
        unsafe { out.write(v) };
        0
    }

    unsafe extern "C" fn viewport_actor(
        _ctx: *mut c_void,
        _client: *mut c_void,
        _index: i32,
        out: *mut *mut c_void,
    ) -> i32 {
        unsafe { out.write(std::ptr::null_mut()) };
        0
    }

    unsafe extern "C" fn line_check(
        ctx: *mut c_void,
        _level: *mut c_void,
        _from: *const Hp2ActorAudioVector,
        _to: *const Hp2ActorAudioVector,
        visible: *mut i32,
    ) -> i32 {
        let c = unsafe { &mut *ctx.cast::<Context>() };
        c.line_check_called = true;
        unsafe { visible.write(1) }; // Skip the radius-shrink branch below.
        0
    }

    unsafe extern "C" fn hear(
        ctx: *mut c_void,
        _hearer: *mut c_void,
        _actor: *mut c_void,
        _id: i32,
        _sound: *mut c_void,
        _location: *const Hp2ActorAudioVector,
        _parameters: *const Hp2ActorAudioVector,
        _disable_3d: i32,
        _looping: i32,
    ) -> i32 {
        let c = unsafe { &mut *ctx.cast::<Context>() };
        c.hear_called = true;
        0
    }

    unsafe extern "C" fn demo(_: *mut c_void, _: *mut c_void, _: *const Hp2ActorAudioSound) -> i32 {
        0
    }
    unsafe extern "C" fn play_music(
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: f32,
        _: *mut i32,
    ) -> i32 {
        0
    }
    unsafe extern "C" fn stop_music(_: *mut c_void, _: *mut c_void, _: i32, _: f32) -> i32 {
        0
    }
    unsafe extern "C" fn stop_all_music(_: *mut c_void, _: *mut c_void, _: f32) -> i32 {
        0
    }
    unsafe extern "C" fn modify_sound(
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: i32,
        _: *mut c_void,
        _: u8,
        _: f32,
        _: *mut i32,
    ) -> i32 {
        0
    }
    unsafe extern "C" fn stop_sound(
        _: *mut c_void,
        _: *mut c_void,
        _: *mut c_void,
        _: i32,
        _: *mut c_void,
        _: f32,
    ) -> i32 {
        0
    }

    fn services(context: *mut c_void) -> Hp2ActorAudioServices {
        Hp2ActorAudioServices {
            context,
            object,
            integer,
            location,
            viewport_actor,
            line_check,
            hear,
            demo,
            play_music,
            stop_music,
            stop_all_music,
            modify_sound,
            stop_sound,
        }
    }

    /// `(HearSource-Location).SizeSquared()` must use the fused formula
    /// (`dz.mul_add(dz, dx.mul_add(dx, dy*dy))`), matching the original's
    /// forward through `UnMath.h` to `hp2_rs_core_math_vector_size_sq`
    /// (`hp2_core::math::dot3`). `dx`/`dy`/`dz` below are chosen so the
    /// fused and plain (`dx*dx+dy*dy+dz*dz`) formulas round to adjacent
    /// f32 values (`0x44eac41d` vs `0x44eac41e`); `radius_squared` is
    /// picked so only the fused result clears the `distance < radius`
    /// check. A regression to the plain formula flips this test from
    /// observing `line_check`/`hear` to not observing them.
    #[test]
    fn distance_uses_fused_multiply_add() {
        let dx = 3.3529413_f32;
        let dy = 40.695652_f32;
        let dz = -14.5172415_f32;
        assert_ne!(
            (dx * dx + dy * dy + dz * dz).to_bits(),
            dz.mul_add(dz, dx.mul_add(dx, dy * dy)).to_bits(),
            "test inputs must actually separate the fused and plain formulas"
        );
        let mut context = Context {
            hearer_loc: Hp2ActorAudioVector {
                x: dx,
                y: dy,
                z: dz,
            },
            actor_loc: Hp2ActorAudioVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            line_check_called: false,
            hear_called: false,
        };
        let s = services((&mut context as *mut Context).cast());
        let radius_squared = 2441.5671_f32;
        let result = unsafe {
            check_hear_sound(
                ACTOR,
                HEARER,
                0,
                std::ptr::null_mut(),
                Hp2ActorAudioVector::default(),
                radius_squared,
                0,
                0,
                &s,
            )
        };
        assert_eq!(result, Ok(()));
        assert!(
            context.line_check_called,
            "fused distance must clear the radius check"
        );
        assert!(context.hear_called);
    }
}
