//! Shared stream contracts: chunk geometry, stream kinds, and the pull-based
//! PCM source trait every decoder feeds.
//!
//! Chunk sizes and buffer counts mirror `ALAudioSubsystem.h` exactly:
//! `OGGVORBIS_STREAM_CHUNKSIZE = 65536`, `XA_STREAM_CHUNKSIZE = 32768`, and
//! three ring buffers per stream (`MAX_BUFFERS_PER_STREAM`).

use crate::error::AudioError;

/// Encoded-stream bytes per Ogg refill chunk (`OGGVORBIS_STREAM_CHUNKSIZE`).
pub const OGG_CHUNK_BYTES: usize = 65536;
/// PCM bytes per XA refill chunk (`XA_STREAM_CHUNKSIZE`).
pub const XA_CHUNK_BYTES: usize = 32768;
/// Ring buffers per stream (`MAX_BUFFERS_PER_STREAM`).
pub const BUFFERS_PER_STREAM: usize = 3;

/// Stream type subset mirroring `EFileStreamType` (the byte-file
/// `ST_Regular` path is plain file IO and stays out of this crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// `ST_Ogg`: non-looping streamed Ogg Vorbis.
    Ogg,
    /// `ST_OggLooping`: music-flagged Ogg Vorbis; wraps to sample zero at EOF.
    OggLooping,
    /// `ST_XA`: non-looping EA-XA ADPCM decoded through `hp_format::eaxa`.
    Xa,
    /// `ST_XALooping`: looping EA-XA; refeds the encoded stream at end.
    XaLooping,
}

impl StreamKind {
    /// Whether EOF wraps back to the beginning of the encoded stream.
    pub fn is_looping(self) -> bool {
        matches!(self, StreamKind::OggLooping | StreamKind::XaLooping)
    }

    /// Refill chunk size in PCM bytes for this stream kind.
    pub fn chunk_bytes(self) -> usize {
        match self {
            StreamKind::Ogg | StreamKind::OggLooping => OGG_CHUNK_BYTES,
            StreamKind::Xa | StreamKind::XaLooping => XA_CHUNK_BYTES,
        }
    }

    /// Refill chunk size in interleaved samples (PCM bytes are two per i16).
    pub fn chunk_samples(self) -> usize {
        self.chunk_bytes() / 2
    }
}

/// Decoder-provided stream metadata handed to sinks when a stream opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamInfo {
    /// Interleaved channel count.
    pub channels: u16,
    /// Playback rate in samples per second.
    pub sample_rate: u32,
}

/// Result of one pull-fill of a [`PcmSource`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fill {
    /// Samples written into the front of the destination buffer.
    pub written: usize,
    /// True once the stream reached terminal EOF: the tail beyond `written`
    /// was zero-filled and the stream will produce nothing further.
    pub eof: bool,
}

/// Pull-based PCM producer mirroring the first-party `ReadStreamLocked`
/// semantics: a fill either fills the whole buffer, or hits terminal EOF,
/// zero-fills the tail, and reports it. Looping sources wrap internally and
/// keep filling. Decode failures are loud [`AudioError`]s, never silence.
pub trait PcmSource: Send {
    /// Stream kind this source decodes.
    fn kind(&self) -> StreamKind;

    /// Decoder metadata (channels/rate).
    fn info(&self) -> StreamInfo;

    /// Fill `out` with interleaved i16 samples.
    fn fill(&mut self, out: &mut [i16]) -> Result<Fill, AudioError>;
}
