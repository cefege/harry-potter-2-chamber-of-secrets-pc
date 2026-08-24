//! rodio output graph, Ogg-loop and XA stream types.
//!
//! Phase 5 audio slice. Stream semantics mirror the first-party ALAudio
//! streaming path (`HarryPotter2/Unreal/ALAudio` + `UnFileStream.cpp`):
//! chunk sizes 65536 (Ogg) / 32768 (XA) bytes, three ring buffers per
//! stream, `ST_OggLooping` wrap-at-EOF and `ST_XA(Looping)` refeed
//! behaviors, loud `audio.*` reason-coded errors throughout.
//!
//! Module map:
//! - [`error`] — reason-code family per `Docs/REASON_CODES.md`.
//! - [`source`] — shared contracts: chunk geometry, stream kinds, PCM pull trait.
//! - [`ogg`] — symphonia-based Vorbis source with libvorbis-exact i16 quantization.
//! - [`xa`] — EA-XA ADPCM source decoded through `hp_format::eaxa`.
//! - [`graph`] — output graph with injectable sinks and a deterministic lifecycle trace.
//! - [`stream`] — slot/ring/completion manager mirroring `FFileStream` bookkeeping.

pub mod error;
pub mod graph;
pub mod ogg;
pub mod source;
pub mod stream;
pub mod xa;

/// Day-one identity marker; replaced by audio-lifecycle tests from Phase 5.
pub const CRATE_NAME: &str = "hp-audio";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-audio");
    }
}
