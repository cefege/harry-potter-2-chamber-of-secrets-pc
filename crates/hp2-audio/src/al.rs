//! OpenAL and ALC constants used by the subsystem, plus the port of
//! `UALAudioSubsystem::alError`.
//!
//! The values are the ones in `HarryPotter2/Unreal/OpenAL/Inc/al.h` and
//! `alctypes.h`. The C++ shell `static_assert`s each of them against the
//! header, so a drifted constant fails the C++ build.

use crate::host::{Host, HostResult};

pub const AL_FALSE: i32 = 0;
pub const AL_TRUE: i32 = 1;
pub const AL_SOURCE_RELATIVE: i32 = 0x202;
pub const AL_PITCH: i32 = 0x1003;
pub const AL_POSITION: i32 = 0x1004;
pub const AL_VELOCITY: i32 = 0x1006;
pub const AL_LOOPING: i32 = 0x1007;
pub const AL_BUFFER: i32 = 0x1009;
pub const AL_GAIN: i32 = 0x100A;
pub const AL_ORIENTATION: i32 = 0x100F;
pub const AL_SOURCE_STATE: i32 = 0x1010;
pub const AL_INITIAL: i32 = 0x1011;
pub const AL_PLAYING: i32 = 0x1012;
pub const AL_PAUSED: i32 = 0x1013;
pub const AL_STOPPED: i32 = 0x1014;
pub const AL_BUFFERS_PROCESSED: i32 = 0x1016;
pub const AL_REFERENCE_DISTANCE: i32 = 0x1020;
pub const AL_ROLLOFF_FACTOR: i32 = 0x1021;
pub const AL_FORMAT_MONO8: i32 = 0x1100;
pub const AL_FORMAT_MONO16: i32 = 0x1101;
pub const AL_FORMAT_STEREO8: i32 = 0x1102;
pub const AL_FORMAT_STEREO16: i32 = 0x1103;
pub const AL_SIZE: i32 = 0x2004;
pub const AL_NO_ERROR: i32 = 0;
pub const AL_INVALID_NAME: i32 = 0xA001;
pub const AL_INVALID_ENUM: i32 = 0xA002;
pub const AL_INVALID_VALUE: i32 = 0xA003;
pub const AL_INVALID_OPERATION: i32 = 0xA004;
pub const AL_OUT_OF_MEMORY: i32 = 0xA005;
pub const AL_INVERSE_DISTANCE_CLAMPED: i32 = 0xD002;
pub const ALC_FREQUENCY: i32 = 0x1007;

/// `UALAudioSubsystem::alError(Text, Log)`. Returns `true` when an AL error
/// is pending. The error is always consumed. The message is logged only
/// when `log` is set.
pub fn al_error<H: Host + ?Sized>(host: &mut H, text: &str, log: bool) -> HostResult<bool> {
    let error = host.al_get_error();
    if error == AL_NO_ERROR {
        return Ok(false);
    }
    if log {
        let message = match error {
            AL_INVALID_NAME => format!("ALAudio: AL_INVALID_NAME in {text}"),
            AL_INVALID_ENUM => format!("ALAudio: AL_INVALID_ENUM in {text}"),
            AL_INVALID_VALUE => format!("ALAudio: AL_INVALID_VALUE in {text}"),
            AL_INVALID_OPERATION => format!("ALAudio: AL_INVALID_OPERATION in {text}"),
            AL_OUT_OF_MEMORY => format!("ALAudio: AL_OUT_OF_MEMORY in {text}"),
            _ => format!("ALAudio: Unknown error in {text}"),
        };
        host.log(crate::host::LogKind::Log, &message)?;
    }
    Ok(true)
}
