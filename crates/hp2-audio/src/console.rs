//! `UALAudioSubsystem::Exec` dispatch, independent of the borrowed audio backend.
//!
//! Native leaves retain typed UObject/global writes and calls back into the audio
//! backend. This orchestrator never acquires a `Subsystem` borrow or arena lock.

use std::ffi::c_void;
use std::ptr::null;

use hp2_core::ansi::Tchar as TChar;
use hp2_core::command::parse_command;

/// cbindgen:prefix-with-name
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum Hp2AudioConsoleLeaf {
    PauseSounds = 0,
    UnpauseSounds = 1,
    RestartSounds = 2,
    SetWeaponRadius = 3,
    SetRollOff = 4,
    StopSounds = 5,
    PlayingSoundStats = 6,
}

#[repr(C)]
pub struct Hp2AudioConsoleServices {
    pub opaque: *mut c_void,
    /// Return zero on success, otherwise the captured native failure status.
    /// `text` is the parsed argument tail for property writes, null otherwise.
    pub invoke: unsafe extern "C" fn(*mut c_void, u32, *const c_void) -> i32,
}

impl Hp2AudioConsoleServices {
    unsafe fn call(&self, leaf: Hp2AudioConsoleLeaf, text: *const TChar) -> Result<(), i32> {
        // SAFETY: the caller supplies a live callback/context that contains all
        // native exceptions; text is null or points into its live command.
        match unsafe { (self.invoke)(self.opaque, leaf as u32, text.cast()) } {
            0 => Ok(()),
            status => Err(status),
        }
    }
}

const fn wide<const N: usize>(bytes: &[u8; N]) -> [TChar; N] {
    let mut result = [0; N];
    let mut index = 0;
    while index < N {
        result[index] = bytes[index] as TChar;
        index += 1;
    }
    result
}

const ASTAT: [TChar; 6] = wide(b"ASTAT\0");
const AUDIO: [TChar; 6] = wide(b"AUDIO\0");
const PAUSESOUNDS: [TChar; 12] = wide(b"PAUSESOUNDS\0");
const UNPAUSESOUNDS: [TChar; 14] = wide(b"UNPAUSESOUNDS\0");
const RESTARTSOUNDS: [TChar; 14] = wide(b"RESTARTSOUNDS\0");
const WEAPONRADIUS: [TChar; 13] = wide(b"WEAPONRADIUS\0");
const ROLLOFF: [TChar; 8] = wide(b"ROLLOFF\0");
const PLAYINGSOUNDSTATS: [TChar; 18] = wide(b"PLAYINGSOUNDSTATS\0");

/// Runs the legacy command chain without allocating or borrowing a subsystem.
///
/// # Safety
/// `services` is a live immutable callback table. A non-null `command` points to
/// a NUL-terminated native TCHAR string valid throughout the call. Callbacks
/// must contain native exceptions, return status, and not invalidate command
/// storage or the services table. A null command is unhandled.
pub unsafe fn exec(
    services: &Hp2AudioConsoleServices,
    mut command: *const TChar,
) -> Result<bool, i32> {
    if command.is_null() {
        return Ok(false);
    }
    let mut str_cursor = command;
    // SAFETY: the caller supplies live terminated command storage and checked
    // callbacks; all matching literals are compile-time terminated strings.
    unsafe {
        if parse_command(&mut str_cursor, ASTAT.as_ptr()) != 0 {
            // An unknown ASTAT subcommand ends this else-if chain; it must not
            // be reinterpreted as a top-level audio command.
            return Ok(parse_command(&mut str_cursor, AUDIO.as_ptr()) != 0);
        } else if parse_command(&mut str_cursor, PAUSESOUNDS.as_ptr()) != 0 {
            services.call(Hp2AudioConsoleLeaf::PauseSounds, null())?;
        } else if parse_command(&mut str_cursor, UNPAUSESOUNDS.as_ptr()) != 0 {
            services.call(Hp2AudioConsoleLeaf::UnpauseSounds, null())?;
        } else if parse_command(&mut str_cursor, RESTARTSOUNDS.as_ptr()) != 0 {
            services.call(Hp2AudioConsoleLeaf::RestartSounds, null())?;
        } else if parse_command(&mut command, WEAPONRADIUS.as_ptr()) != 0 {
            // These final three branches use the original Cmd cursor, not Str.
            services.call(Hp2AudioConsoleLeaf::SetWeaponRadius, command)?;
        } else if parse_command(&mut command, ROLLOFF.as_ptr()) != 0 {
            services.call(Hp2AudioConsoleLeaf::SetRollOff, command)?;
            // Publish the UObject property before StopSound can reenter audio.
            services.call(Hp2AudioConsoleLeaf::StopSounds, null())?;
        } else if parse_command(&mut command, PLAYINGSOUNDSTATS.as_ptr()) != 0 {
            services.call(Hp2AudioConsoleLeaf::PlayingSoundStats, null())?;
        } else {
            return Ok(false);
        }
    }
    Ok(true)
}
