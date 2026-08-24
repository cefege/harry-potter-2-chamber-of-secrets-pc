//! EA-XA streamed PCM source (`ST_XA` / `ST_XALooping`).
//!
//! Pull-side port of the first-party `ReadXALocked`/`RefeedXA` loop
//! (`HarryPotter2/Unreal/Core/Src/UnFileStream.cpp` lines 208-263):
//! partial destination fills continue from the decoder residue without
//! dropping samples, a non-looping stream zero-fills its tail and goes
//! terminally silent at end of stream, and a looping stream refeeds the very
//! same encoded bytes — preserving predictor history across the wrap, exactly
//! like `RefeedXA`. A second consecutive wrap that still produces nothing is
//! a loud `audio.loop_stalled` failure, never silence.
//!
//! Errors carry `audio.*` reason codes (see [`crate::error`]); encoder-block
//! rejections surface as `audio.eaxa_feed_failed` wrapping the underlying
//! `eaxa.*` error. No path ever falls back silently.

use crate::error::AudioError;
use crate::source::{Fill, PcmSource, StreamInfo, StreamKind};
use hp_format::eaxa::EaxaDecoder;

/// Streaming EA-XA ADPCM source mirroring `FXAStreamState` + `ReadXALocked`.
#[derive(Debug)]
pub struct XaSource {
    /// Encoded block stream kept verbatim for looping refeeds (`RefeedXA`
    /// feeds the identical buffer every pass).
    encoded: Vec<u8>,
    /// Promised sample count handed to every feed (`Stream.NumSamples`).
    num_samples: usize,
    kind: StreamKind,
    sample_rate: u32,
    /// Block decoder; a fresh instance carries zero predictor history, which
    /// is the `ResetState`-equivalent of `CreateXALocked`.
    decoder: EaxaDecoder,
    /// Set after a refeed that produced no forward progress; a second
    /// fruitless decode aborts loudly instead of spinning forever.
    refeed_without_progress: bool,
}

impl XaSource {
    /// Mirrors `CreateXALocked`: copies the encoded bytes, resets decoder
    /// state (a fresh [`EaxaDecoder`] has zero history), and performs the
    /// initial feed. Feed rejection surfaces immediately as
    /// [`AudioError::EaxaFeedFailed`] — construction never yields a dead
    /// stream.
    pub fn new(
        encoded: Vec<u8>,
        num_samples: usize,
        sample_rate: u32,
        kind: StreamKind,
    ) -> Result<Self, AudioError> {
        if !matches!(kind, StreamKind::Xa | StreamKind::XaLooping) {
            return Err(AudioError::BadRequest {
                detail: format!(
                    "XA source requires StreamKind::Xa or StreamKind::XaLooping, got {kind:?}"
                ),
            });
        }
        let mut decoder = EaxaDecoder::new();
        decoder
            .feed(&encoded, num_samples)
            .map_err(|source| AudioError::EaxaFeedFailed { source })?;
        Ok(Self {
            encoded,
            num_samples,
            kind,
            sample_rate,
            decoder,
            refeed_without_progress: false,
        })
    }

    /// `RefeedXA`: feeds the same encoded bytes and sample promise again.
    /// Decoder history persists across feeds by design — the wrap must splice
    /// seamlessly onto the previous pass, not restart the predictors.
    fn refeed(&mut self) -> Result<(), AudioError> {
        self.decoder
            .feed(&self.encoded, self.num_samples)
            .map_err(|source| AudioError::EaxaFeedFailed { source })
    }
}

impl PcmSource for XaSource {
    fn kind(&self) -> StreamKind {
        self.kind
    }

    /// EA-XA is mono (`CreateXALocked` streams one interleaved channel).
    fn info(&self) -> StreamInfo {
        StreamInfo {
            channels: 1,
            sample_rate: self.sample_rate,
        }
    }

    /// Mirrors `ReadXALocked`: decode into the unfilled tail until the caller
    /// request is satisfied, end of stream arrives, or a looping wrap stalls.
    fn fill(&mut self, out: &mut [i16]) -> Result<Fill, AudioError> {
        // ReadStreamLocked short-circuits zero-byte requests before touching
        // decoder state; an empty fill must not fabricate an EOF.
        if out.is_empty() {
            return Ok(Fill {
                written: 0,
                eof: false,
            });
        }

        let mut written = 0usize;
        while written < out.len() {
            let produced = self.decoder.decode(&mut out[written..]).map_err(|err| {
                // Feed-time rejects were already surfaced at construction /
                // refeed; anything escaping decode is a malformed accepted
                // stream and passes through as a bad request.
                AudioError::BadRequest {
                    detail: err.to_string(),
                }
            })?;
            written += produced;

            if produced > 0 {
                self.refeed_without_progress = false;
                continue;
            }

            if !self.kind.is_looping() {
                // Terminal EOF (`ST_XA`): zero-fill the remainder exactly like
                // ReadXALocked's memset and report the buffer fully populated.
                out[written..].fill(0);
                return Ok(Fill {
                    written: out.len(),
                    eof: true,
                });
            }

            if self.refeed_without_progress {
                // A wrap that still decodes nothing means the stream cannot
                // sustain its loop; first-party returns failure here too.
                return Err(AudioError::LoopStalled {
                    detail: format!(
                        "looping EA-XA stream wrapped twice without producing samples \
                         ({} promised samples, {} encoded blocks)",
                        self.num_samples,
                        self.encoded.len() / hp_format::eaxa::BYTES_PER_BLOCK,
                    ),
                });
            }

            self.refeed()?;
            self.refeed_without_progress = true;
        }

        Ok(Fill {
            written,
            eof: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp_format::eaxa::{BYTES_PER_BLOCK, SAMPLES_PER_BLOCK};

    const SAMPLE_RATE: u32 = 22050;

    /// Synthetic block straight from `Tests/AudioTests.cpp` `FillXABlock`:
    /// byte 0 = predictor 0 / range 4, bytes 1..15 = `(i << 4) | (15 - i)`.
    fn synthetic_block() -> Vec<u8> {
        let mut raw = vec![0u8; BYTES_PER_BLOCK];
        raw[0] = 0x04;
        for (index, byte) in raw.iter_mut().enumerate().skip(1) {
            *byte = ((index << 4) | (BYTES_PER_BLOCK - 1 - index)) as u8;
        }
        raw
    }

    /// Reference decode through a bare `EaxaDecoder`, matching
    /// `DecodeExpectedXA` in `Tests/AudioTests.cpp`.
    fn expected_samples(raw: &[u8]) -> Vec<i16> {
        let mut decoder = EaxaDecoder::new();
        decoder
            .feed(raw, SAMPLES_PER_BLOCK)
            .expect("synthetic XA block feeds reference decoder");
        decoder.decode_to_end().expect("synthetic XA block decodes")
    }

    #[test]
    fn partial_fills_continue_residue_without_dropping() {
        let raw = synthetic_block();
        let expected = expected_samples(&raw);
        let mut source = XaSource::new(raw.clone(), SAMPLES_PER_BLOCK, SAMPLE_RATE, StreamKind::Xa)
            .expect("valid single-block stream constructs");

        let mut first = [1234i16; 5];
        let fill = source.fill(&mut first).expect("partial fill succeeds");
        assert_eq!(fill.written, 5);
        assert!(!fill.eof);
        assert_eq!(&first, &expected[..5]);

        let mut second = [0i16; 10];
        let fill = source.fill(&mut second).expect("second partial succeeds");
        assert_eq!(fill.written, 10);
        assert!(!fill.eof);
        assert_eq!(&second, &expected[5..15]);
    }

    #[test]
    fn odd_request_lengths_tile_the_stream_exactly() {
        let raw = synthetic_block();
        let expected = expected_samples(&raw);
        let mut source = XaSource::new(raw.clone(), SAMPLES_PER_BLOCK, SAMPLE_RATE, StreamKind::Xa)
            .expect("valid single-block stream constructs");

        let mut decoded = Vec::new();
        for chunk_len in [7usize, 13, 1, 9] {
            let mut chunk = vec![0i16; chunk_len];
            let fill = source.fill(&mut chunk).expect("odd-sized fill succeeds");
            assert_eq!(fill.written, chunk_len);
            // 7 + 13 + 1 + 9 = 30 exceeds the 28-sample stream: the final
            // tile lands past EOF and zero-fills its overflow.
            let crosses_eof = decoded.len() + chunk_len > SAMPLES_PER_BLOCK;
            assert_eq!(fill.eof, crosses_eof);
            if crosses_eof {
                assert!(
                    chunk[SAMPLES_PER_BLOCK - decoded.len()..]
                        .iter()
                        .all(|&s| s == 0),
                    "overflow past EOF is zero-filled"
                );
            }
            decoded.extend_from_slice(&chunk);
        }
        assert_eq!(&decoded[..SAMPLES_PER_BLOCK], &expected[..]);
    }

    #[test]
    fn eof_preserves_samples_and_zero_fills_the_tail() {
        let raw = synthetic_block();
        let expected = expected_samples(&raw);
        let mut source = XaSource::new(raw.clone(), SAMPLES_PER_BLOCK, SAMPLE_RATE, StreamKind::Xa)
            .expect("valid single-block stream constructs");

        let mut out = vec![1234i16; 34];
        let fill = source.fill(&mut out).expect("eof fill succeeds");
        assert!(fill.eof, "non-looping exhaustion is terminal");
        assert_eq!(fill.written, out.len(), "buffer is fully populated on Ok");
        assert_eq!(&out[..28], &expected[..]);
        assert!(
            out[28..].iter().all(|&s| s == 0),
            "tail beyond the stream must be zero-filled"
        );

        // Past-terminal fills stay silent and keep reporting EOF.
        let mut again = vec![7i16; 4];
        let fill = source.fill(&mut again).expect("post-eof fill succeeds");
        assert!(fill.eof);
        assert!(again.iter().all(|&s| s == 0));
    }

    #[test]
    fn looping_refills_past_eof_from_encoded_beginning() {
        let raw = synthetic_block();
        let expected = expected_samples(&raw);
        let mut source = XaSource::new(
            raw.clone(),
            SAMPLES_PER_BLOCK,
            SAMPLE_RATE,
            StreamKind::XaLooping,
        )
        .expect("valid single-block loop constructs");

        // First pass: exactly the 28 promised samples...
        let mut out = vec![0i16; 34];
        let fill = source.fill(&mut out).expect("looping first pass succeeds");
        assert!(!fill.eof, "looping streams never report terminal EOF");
        assert_eq!(&out[..28], &expected[..]);

        // ...then the refeed splices a fresh pass onto the residue.
        assert_eq!(&out[28..], &expected[..6]);

        // The continuation lines up with the reference stream, offset by the
        // six samples already emitted from pass two.
        let mut next = vec![0i16; 22];
        let fill = source
            .fill(&mut next)
            .expect("looping continuation succeeds");
        assert!(!fill.eof);
        assert_eq!(&next, &expected[6..]);
    }

    #[test]
    fn looping_wrap_that_produces_nothing_is_loud() {
        // Zero promised samples: every refeed succeeds but decodes nothing,
        // so the once-guard must trip into LoopStalled instead of spinning.
        let raw = synthetic_block();
        let mut source = XaSource::new(raw.clone(), 0, SAMPLE_RATE, StreamKind::XaLooping)
            .expect("zero-sample loop constructs");

        let mut out = [0i16; 16];
        let err = source.fill(&mut out).expect_err("stalled loop must fail");
        assert_eq!(err.reason_code(), "audio.loop_stalled");
        assert!(matches!(err, AudioError::LoopStalled { .. }));
    }

    #[test]
    fn non_multiple_of_block_size_feeds_fail_loudly_at_construction() {
        let mut raw = synthetic_block();
        raw.truncate(BYTES_PER_BLOCK - 1);
        let err = XaSource::new(raw, SAMPLES_PER_BLOCK, SAMPLE_RATE, StreamKind::Xa)
            .expect_err("ragged feed must be rejected");
        assert_eq!(err.reason_code(), "audio.eaxa_feed_failed");
        assert!(matches!(err, AudioError::EaxaFeedFailed { .. }));
    }

    #[test]
    fn sample_promise_beyond_encoded_capacity_is_rejected() {
        let raw = synthetic_block();
        let err = XaSource::new(raw, SAMPLES_PER_BLOCK + 1, SAMPLE_RATE, StreamKind::Xa)
            .expect_err("over-promised stream must be rejected");
        assert_eq!(err.reason_code(), "audio.eaxa_feed_failed");
    }

    #[test]
    fn wrong_stream_kind_is_a_bad_request() {
        let raw = synthetic_block();
        for kind in [StreamKind::Ogg, StreamKind::OggLooping] {
            let err = XaSource::new(raw.clone(), SAMPLES_PER_BLOCK, SAMPLE_RATE, kind)
                .expect_err("non-XA kinds are rejected");
            assert_eq!(err.reason_code(), "audio.bad_request");
        }
    }

    #[test]
    fn metadata_reports_mono_and_requested_rate() {
        let raw = synthetic_block();
        let source = XaSource::new(raw.clone(), SAMPLES_PER_BLOCK, 44_100, StreamKind::Xa)
            .expect("valid stream constructs");
        let info = source.info();
        assert_eq!(info.channels, 1);
        assert_eq!(info.sample_rate, 44_100);
        assert_eq!(source.kind(), StreamKind::Xa);
    }

    #[test]
    fn empty_fill_neither_decodes_nor_invents_eof() {
        let raw = synthetic_block();
        let mut source = XaSource::new(raw.clone(), SAMPLES_PER_BLOCK, SAMPLE_RATE, StreamKind::Xa)
            .expect("valid stream constructs");

        let fill = source.fill(&mut []).expect("empty fill succeeds");
        assert_eq!(fill.written, 0);
        assert!(!fill.eof);

        // The untouched stream still delivers its full payload afterwards.
        let expected = expected_samples(&raw);
        let mut out = vec![0i16; SAMPLES_PER_BLOCK];
        source
            .fill(&mut out)
            .expect("stream intact after empty fill");
        assert_eq!(out, expected);
    }
}
