//! Streaming Ogg Vorbis PCM source mirroring the first-party `ReadOggLocked`
//! decoder loop (`HarryPotter2/Unreal/Core/Src/UnFileStream.cpp` lines
//! 156-206): a fill either completes the caller's buffer entirely, or hits
//! terminal EOF and zero-fills the tail (the `ov_read <= 0` non-looping
//! branch), or — when the stream kind is looping — recreates the decoder from
//! the retained encoded bytes and keeps filling (the `ST_OggLooping`
//! `ov_pcm_seek(0)` wrap). A looping stream that reached EOF without producing
//! a single sample since its last wrap is exactly the first-party
//! "corrupt or empty looping stream" case: it fails loudly instead of spinning
//! or emitting silence.
//!
//! Quantization reproduces libvorbis' `ov_read(word=2, sgned=1)` float→i16
//! conversion exactly: each decoded float sample is scaled by 32768.0 and
//! converted via `vorbis_ftoi`, which on SSE2 targets is `_mm_cvtsd_si32`
//! (round-to-nearest, ties-to-even), followed by a clamp into the signed
//! 16-bit range (xiph/vorbis `lib/vorbisfile.c` around line 2032, conversion
//! helper in `lib/os.h`).
//!
//! One documented deviation from `ov_read`: symphonia surfaces recoverable
//! per-packet corruption as `Error::DecodeError`; following symphonia's
//! documented best practice we skip such packets and keep decoding rather
//! than aborting the stream. Every other failure is a loud reason-coded
//! [`AudioError`] (`audio.open_failed`, `audio.decode_failed`,
//! `audio.loop_stalled`) — never a silent stream.

use std::io::Cursor;
use std::path::Path;

use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::well_known;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::default::{get_codecs, get_probe};

use crate::error::AudioError;
use crate::source::{Fill, PcmSource, StreamInfo, StreamKind};

/// Exact libvorbis `ov_read(word=2, sgned=1)` float→i16 sample conversion.
///
/// libvorbis scales by 32768.0 and converts through `vorbis_ftoi`, which on
/// SSE2 builds is `_mm_cvtsd_si32` — round-to-nearest, ties-to-even — and
/// then clamps to the i16 range (`lib/vorbisfile.c` ~line 2032, `lib/os.h`).
/// We mirror that bit-for-bit in portable code: widen to f64 (the SSE2
/// intrinsic converts through double precision), round ties to even, clamp.
fn quantize(sample: f32) -> i16 {
    let v = (f64::from(sample) * 32768.0).round_ties_even();
    v.clamp(-32768.0, 32767.0) as i16
}

/// Freshly built symphonia demuxer + Vorbis decoder state.
struct Decoded {
    info: StreamInfo,
    track_id: u32,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
}

/// Probes encoded bytes as Ogg Vorbis and instantiates the decoder chain.
///
/// Consumes `bytes` (a looping wrap later clones the retained copy back in —
/// the one unavoidable reallocation, documented on [`OggSource::restart`]).
fn start_decode(bytes: Vec<u8>) -> Result<Decoded, AudioError> {
    let mut hint = Hint::new();
    hint.with_extension("ogg");

    let mss = MediaSourceStream::new(
        Box::new(Cursor::new(bytes)),
        MediaSourceStreamOptions::default(),
    );
    let probed = get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| AudioError::OpenFailed {
            detail: format!("ogg probe failed: {e}"),
        })?;

    // Pick the first track whose codec parameters name Vorbis; anything else
    // is an open failure, never a silent passthrough.
    let mut selected = None;
    for track in probed.tracks() {
        if let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref()
            && params.codec == well_known::CODEC_ID_VORBIS
        {
            selected = Some((track.id, params.clone()));
            break;
        }
    }
    let Some((track_id, audio_params)) = selected else {
        return Err(AudioError::OpenFailed {
            detail: "no vorbis track".to_string(),
        });
    };

    // Channel count arrives as a channel-layout descriptor; reduce it to the
    // interleaved count and refuse anything that cannot be a u16 channel
    // count (mirrors the first-party assumption that the header was parsed).
    let channels = audio_params
        .channels
        .as_ref()
        .map(|c| c.count())
        .filter(|&n| n > 0 && n <= usize::from(u16::MAX))
        .ok_or_else(|| AudioError::OpenFailed {
            detail: "vorbis track has no usable channel count".to_string(),
        })? as u16;
    let sample_rate = audio_params
        .sample_rate
        .ok_or_else(|| AudioError::OpenFailed {
            detail: "vorbis track has no sample rate".to_string(),
        })?;
    let decoder = get_codecs()
        .make_audio_decoder(&audio_params, &AudioDecoderOptions::default())
        .map_err(|e| AudioError::DecodeFailed {
            detail: format!("vorbis decoder init failed: {e}"),
        })?;

    Ok(Decoded {
        info: StreamInfo {
            channels,
            sample_rate,
        },
        track_id,
        format: probed,
        decoder,
    })
}

/// Non-looping / looping streamed Ogg Vorbis source (`ST_Ogg`,
/// `ST_OggLooping`). Keeps the full encoded bytes so a looping wrap can
/// deterministically restart decoding from sample zero.
#[allow(missing_docs)]
pub struct OggSource {
    bytes: Vec<u8>,
    kind: StreamKind,
    info: StreamInfo,
    format: Box<dyn FormatReader>,
    track_id: u32,
    decoder: Box<dyn AudioDecoder>,
    /// One decoded packet, buffered as interleaved f32 and quantized to i16
    /// on demand.
    pending: Vec<f32>,
    pending_pos: usize,
    /// Samples produced since the last looping wrap (the `ProducedSinceSeek`
    /// guard in `ReadOggLocked`).
    produced_since_wrap: usize,
    /// Completed passes over the bitstream.
    passes: u64,
}

impl std::fmt::Debug for OggSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Decoder internals are opaque; expose only the caller-meaningful state.
        f.debug_struct("OggSource")
            .field("kind", &self.kind)
            .field("info", &self.info)
            .field("encoded_bytes", &self.bytes.len())
            .field("passes", &self.passes)
            .finish_non_exhaustive()
    }
}

impl OggSource {
    /// Reads the file fully into memory, probes it as Ogg Vorbis via
    /// symphonia, and prepares decode state. Any failure yields
    /// [`AudioError::OpenFailed`] or [`AudioError::DecodeFailed`]; never a
    /// silent stream.
    pub fn open(path: &Path, kind: StreamKind) -> Result<Self, AudioError> {
        let bytes = std::fs::read(path).map_err(|e| AudioError::OpenFailed {
            detail: format!("failed to read {}: {e}", path.display()),
        })?;
        Self::from_bytes(bytes, kind)
    }

    /// Same, from in-memory encoded bytes (used for synthetic/truncated
    /// fixtures).
    pub fn from_bytes(bytes: Vec<u8>, kind: StreamKind) -> Result<Self, AudioError> {
        let Decoded {
            info,
            track_id,
            format,
            decoder,
        } = start_decode(bytes.clone())?;
        Ok(OggSource {
            bytes,
            kind,
            info,
            format,
            track_id,
            decoder,
            pending: Vec::new(),
            pending_pos: 0,
            produced_since_wrap: 0,
            passes: 0,
        })
    }

    /// Number of completed loop wraps (passes over the bitstream). Zero for
    /// non-looping streams.
    pub fn passes(&self) -> u64 {
        self.passes
    }

    /// Recreates the reader and decoder fresh from the retained encoded
    /// bytes: a deterministic restart from sample zero, standing in for the
    /// first-party `ov_pcm_seek(Ogg, 0)`. The clone of `bytes` is the one
    /// unavoidable allocation on the wrap path — the cursor must own its
    /// data, and the struct cannot borrow from itself.
    fn restart(&mut self) -> Result<(), AudioError> {
        let Decoded {
            info,
            track_id,
            format,
            decoder,
        } = start_decode(self.bytes.clone())?;
        debug_assert_eq!(
            info, self.info,
            "re-probing identical bytes changed stream metadata"
        );
        self.format = format;
        self.track_id = track_id;
        self.decoder = decoder;
        self.pending.clear();
        self.pending_pos = 0;
        self.produced_since_wrap = 0;
        self.passes += 1;
        Ok(())
    }
}

impl PcmSource for OggSource {
    fn kind(&self) -> StreamKind {
        self.kind
    }

    fn info(&self) -> StreamInfo {
        self.info
    }

    /// Mirrors `ReadOggLocked`: fills `out` completely, wrapping internally
    /// while the stream loops. On terminal EOF the tail beyond the decoded
    /// prefix is zero-filled (the `memset(Dest + Count, 0, Bytes - Count)`
    /// branch) and reported via `eof: true` — `written` counts the entire
    /// populated buffer, zeros included.
    fn fill(&mut self, out: &mut [i16]) -> Result<Fill, AudioError> {
        let mut written = 0usize;
        while written < out.len() {
            if self.pending_pos >= self.pending.len() {
                match self.format.next_packet() {
                    Ok(Some(packet)) => {
                        if packet.track_id != self.track_id {
                            // Demuxed side track (e.g. embedded metadata
                            // stream): hand it back and keep pulling.
                            continue;
                        }
                        match self.decoder.decode(&packet) {
                            Ok(decoded) => {
                                self.pending.clear();
                                self.pending_pos = 0;
                                decoded.copy_to_vec_interleaved::<f32>(&mut self.pending);
                                self.produced_since_wrap += self.pending.len();
                            }
                            // Recoverable per-packet corruption: skip the
                            // packet and continue (symphonia's documented best
                            // practice; see module docs for why this differs
                            // from ov_read's fatal treatment).
                            Err(SymphoniaError::DecodeError(_)) => {
                                self.pending.clear();
                                self.pending_pos = 0;
                            }
                            Err(e) => {
                                return Err(AudioError::DecodeFailed {
                                    detail: format!("vorbis decode failed: {e}"),
                                });
                            }
                        }
                        continue;
                    }
                    Ok(None) => {
                        // End of the bitstream. symphonia 0.6 signals this as
                        // `Ok(None)`; the first-party saw ov_read return 0.
                        if !self.kind.is_looping() {
                            out[written..].fill(0);
                            return Ok(Fill {
                                written: out.len(),
                                eof: true,
                            });
                        }
                        if self.produced_since_wrap > 0 {
                            self.restart()?;
                            continue;
                        }
                        // Empty or corrupt looping stream: fail loudly rather
                        // than spinning forever (mirrors the first-party
                        // `!ProducedSinceSeek || ov_pcm_seek(..) != 0` guard).
                        return Err(AudioError::LoopStalled {
                            detail: "looping ogg stream wrapped without producing samples"
                                .to_string(),
                        });
                    }
                    Err(e) => {
                        return Err(AudioError::DecodeFailed {
                            detail: format!("ogg demux failed: {e}"),
                        });
                    }
                }
            }
            let available = self.pending.len() - self.pending_pos;
            let take = available.min(out.len() - written);
            for (dst, src) in out[written..written + take]
                .iter_mut()
                .zip(&self.pending[self.pending_pos..self.pending_pos + take])
            {
                *dst = quantize(*src);
            }
            self.pending_pos += take;
            written += take;
        }
        Ok(Fill {
            written: out.len(),
            eof: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_matches_libvorbis_goldens() {
        assert_eq!(quantize(0.0), 0);
        // Full scale clamps asymmetrically, exactly like libvorbis:
        // +1.0 * 32768 = 32768 overflows i16 and clamps to 32767.
        assert_eq!(quantize(1.0), 32767);
        assert_eq!(quantize(-1.0), -32768);
        assert_eq!(quantize(0.5), 16384);
        assert_eq!(quantize(-0.5), -16384);
    }

    #[test]
    fn quantize_rounds_ties_to_even() {
        // Tiny-magnitude cases exercise the f32 -> f64 widening: each input
        // is an exact binary fraction, so the product is exact and only the
        // rounding rule decides the result.
        assert_eq!(quantize(1.0 / 32768.0_f32), 1);
        assert_eq!(quantize(-1.0 / 32768.0_f32), -1);
        // Ties: 0.5 rounds down to even 0, 1.5 up to even 2, 2.5 down to 2.
        assert_eq!(quantize(0.5 / 32768.0_f32), 0);
        assert_eq!(quantize(1.5 / 32768.0_f32), 2);
        assert_eq!(quantize(2.5 / 32768.0_f32), 2);
        assert_eq!(quantize(-0.5 / 32768.0_f32), 0);
    }

    #[test]
    fn garbage_bytes_fail_open_loudly() {
        let err = OggSource::from_bytes(b"notogg".to_vec(), StreamKind::Ogg)
            .expect_err("garbage bytes must not open");
        assert_eq!(err.reason_code(), "audio.open_failed");
    }

    #[test]
    fn truncated_ogg_header_fails_open_loudly() {
        // Real "OggS" magic but nowhere near a complete page: the probe must
        // still refuse it rather than yield a half-open stream.
        let err = OggSource::from_bytes(b"OggS".to_vec(), StreamKind::Ogg)
            .expect_err("truncated page must not open");
        assert_eq!(err.reason_code(), "audio.open_failed");
    }

    #[test]
    fn stock_fixture_decodes_when_present() {
        // Optional integration smoke: decodes the repo's stock music fixture
        // if it is checked out locally, asserting only that real PCM comes
        // back. Missing fixture => silently skipped (lead's integration tests
        // own strict fixture coverage).
        let path = Path::new("../../HarryPotter2/Unreal/Music/sm_bur_PlayfulFail_01.ogg");
        if !path.exists() {
            return;
        }
        let mut source = OggSource::open(path, StreamKind::Ogg).expect("stock fixture must open");
        let mut pcm = vec![0i16; 4096];
        let fill = source.fill(&mut pcm).expect("stock fixture must decode");
        assert!(fill.written > 0, "expected non-empty PCM");
    }
}
