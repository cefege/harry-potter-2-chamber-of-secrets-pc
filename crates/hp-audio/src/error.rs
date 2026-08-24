//! Loud reason-coded errors for the audio subsystem.
//!
//! Every failure renders with an `audio.*` reason-code prefix so callers can
//! surface it through the standard failure-marker pipeline
//! (`Docs/REASON_CODES.md` conventions: `<domain>.<specific>`, dotted,
//! lowercase). Nothing in this crate fails silently.

/// Audio subsystem rejection reasons. Each renders with its `audio.*` code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// `audio.open_failed`: media could not be read, probed, or lacks a Vorbis track.
    OpenFailed { detail: String },
    /// `audio.decode_failed`: decoder hit unrecoverable corruption mid-stream.
    DecodeFailed { detail: String },
    /// `audio.loop_stalled`: a looping stream wrapped without producing samples.
    LoopStalled { detail: String },
    /// `audio.stream_not_alive`: operation on a destroyed or exhausted slot.
    StreamNotAlive { id: usize },
    /// `audio.slot_exhausted`: no free stream slot in the manager.
    SlotExhausted,
    /// `audio.sink_unavailable`: output device could not be opened.
    SinkUnavailable { detail: String },
    /// `audio.bad_request`: invalid argument (empty buffer, zero chunk, ...).
    BadRequest { detail: String },
    /// `audio.eaxa_feed_failed`: EA-XA encoded stream rejected by the block decoder.
    EaxaFeedFailed { source: hp_format::eaxa::EaxaError },
}

impl AudioError {
    /// The dotted reason code for this error, per `Docs/REASON_CODES.md`.
    pub fn reason_code(&self) -> &'static str {
        match self {
            AudioError::OpenFailed { .. } => "audio.open_failed",
            AudioError::DecodeFailed { .. } => "audio.decode_failed",
            AudioError::LoopStalled { .. } => "audio.loop_stalled",
            AudioError::StreamNotAlive { .. } => "audio.stream_not_alive",
            AudioError::SlotExhausted => "audio.slot_exhausted",
            AudioError::SinkUnavailable { .. } => "audio.sink_unavailable",
            AudioError::BadRequest { .. } => "audio.bad_request",
            AudioError::EaxaFeedFailed { .. } => "audio.eaxa_feed_failed",
        }
    }
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::OpenFailed { detail } => write!(f, "audio.open_failed: {detail}"),
            AudioError::DecodeFailed { detail } => write!(f, "audio.decode_failed: {detail}"),
            AudioError::LoopStalled { detail } => write!(f, "audio.loop_stalled: {detail}"),
            AudioError::StreamNotAlive { id } => {
                write!(
                    f,
                    "audio.stream_not_alive: stream {id} is destroyed or at EOF"
                )
            }
            AudioError::SlotExhausted => {
                write!(f, "audio.slot_exhausted: no free stream slot")
            }
            AudioError::SinkUnavailable { detail } => {
                write!(f, "audio.sink_unavailable: {detail}")
            }
            AudioError::BadRequest { detail } => write!(f, "audio.bad_request: {detail}"),
            AudioError::EaxaFeedFailed { source } => {
                write!(f, "audio.eaxa_feed_failed: {source}")
            }
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AudioError::EaxaFeedFailed { source } => Some(source),
            _ => None,
        }
    }
}
