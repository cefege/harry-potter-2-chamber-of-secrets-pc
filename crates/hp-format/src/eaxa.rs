//! EA-XA (extended-XA) mono ADPCM block decoder.
//!
//! Clean-room port of the block layout documented by this repository's own
//! first-party decoder unit (`Tests/EaxaTests.cpp` contracts): each 15-byte
//! block expands to up to 28 signed 16-bit samples. Byte 0 packs the
//! predictor (high nibble, indexing the 16x8 coefficient table cited from
//! pinned vgmstream commit 11508e91) and the range exponent (low nibble);
//! bytes 1..15 pack 28 signed 4-bit nibbles, low nibble first.
//!
//! Errors are loud and carry `eaxa.*` reason-code prefixes so callers can
//! surface them through the standard failure-marker pipeline instead of
//! silently skipping data.

/// Encoded bytes per EA-XA block.
pub const BYTES_PER_BLOCK: usize = 15;
/// Maximum samples produced per EA-XA block.
pub const SAMPLES_PER_BLOCK: usize = 28;

/// Predictor coefficient table: `[c0, c1]` per predictor index 0..16.
const COEFFICIENTS: [[i64; 2]; 16] = [
    [0, 0],
    [240, 0],
    [460, -208],
    [392, -220],
    [488, -240],
    [328, -208],
    [440, -168],
    [420, -188],
    [432, -176],
    [240, -16],
    [416, -192],
    [424, -160],
    [288, -8],
    [436, -188],
    [224, -1],
    [272, -16],
];

/// Decoder rejection reasons. Each renders with its `eaxa.*` reason code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EaxaError {
    /// `eaxa.block_size`: fed byte count is not a multiple of the block size.
    BadBlockLength { got: usize },
    /// `eaxa.capacity`: requested sample count exceeds encoded capacity.
    CapacityExceeded { requested: usize, capacity: usize },
    /// `eaxa.end_of_stream`: decode attempted past the end of the stream.
    EndOfStream,
}

impl std::fmt::Display for EaxaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EaxaError::BadBlockLength { got } => write!(
                f,
                "eaxa.block_size: fed {got} bytes, not a multiple of {BYTES_PER_BLOCK}"
            ),
            EaxaError::CapacityExceeded {
                requested,
                capacity,
            } => write!(
                f,
                "eaxa.capacity: requested {requested} samples, stream holds {capacity}"
            ),
            EaxaError::EndOfStream => write!(f, "eaxa.end_of_stream: no samples remain"),
        }
    }
}

impl std::error::Error for EaxaError {}

/// Streaming mono EA-XA block decoder.
///
/// Mirrors the first-party contract exactly: `feed` is transactional (a
/// rejected feed never disturbs the previously accepted stream), decode
/// requests are capped by both the caller's buffer and the fed sample count,
/// and decoder history persists across feeds.
#[derive(Debug, Default, Clone)]
pub struct EaxaDecoder {
    encoded: Vec<u8>,
    /// Samples the accepted stream promised; decoding stops once consumed.
    stream_samples_remaining: usize,
    encoded_offset: usize,
    residue: [i16; SAMPLES_PER_BLOCK],
    residue_offset: usize,
    residue_len: usize,
    history1: i16,
    history2: i16,
}

impl EaxaDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds the two-sample predictor history (e.g. from a preceding frame).
    pub fn reset_history(&mut self, sample1: i16, sample2: i16) {
        self.history1 = sample1;
        self.history2 = sample2;
    }

    /// Installs a new encoded stream. Transactional: on error the previous
    /// stream (including offsets and residue) stays untouched.
    ///
    /// An empty slice with zero requested samples is legal and decodes to
    /// immediate end-of-stream.
    pub fn feed(&mut self, data: &[u8], num_samples: usize) -> Result<(), EaxaError> {
        if !data.len().is_multiple_of(BYTES_PER_BLOCK) {
            return Err(EaxaError::BadBlockLength { got: data.len() });
        }
        let capacity = data.len() / BYTES_PER_BLOCK * SAMPLES_PER_BLOCK;
        if num_samples > capacity {
            return Err(EaxaError::CapacityExceeded {
                requested: num_samples,
                capacity,
            });
        }
        self.encoded.clear();
        self.encoded.extend_from_slice(data);
        self.stream_samples_remaining = num_samples;
        self.encoded_offset = 0;
        self.residue_offset = 0;
        self.residue_len = 0;
        Ok(())
    }

    /// Decodes up to `out.len()` samples, returning how many were written.
    /// Returns `Ok(0)` once the fed stream is exhausted (end of stream).
    pub fn decode(&mut self, out: &mut [i16]) -> Result<usize, EaxaError> {
        let mut produced = 0usize;
        while produced < out.len() {
            if self.residue_offset < self.residue_len {
                let available = self.residue_len - self.residue_offset;
                let want = out.len() - produced;
                let copy = want.min(available);
                out[produced..produced + copy].copy_from_slice(
                    &self.residue[self.residue_offset..self.residue_offset + copy],
                );
                produced += copy;
                self.residue_offset += copy;
                continue;
            }

            if self.stream_samples_remaining == 0
                || self.encoded_offset + BYTES_PER_BLOCK > self.encoded.len()
            {
                break;
            }

            let block = &self.encoded[self.encoded_offset..self.encoded_offset + BYTES_PER_BLOCK];
            let predictor = (block[0] >> 4) as usize;
            let range = (block[0] & 15) as u32;
            let [c0, c1] = COEFFICIENTS[predictor];
            let block_samples = self.stream_samples_remaining.min(SAMPLES_PER_BLOCK);

            for index in 0..block_samples {
                let packed = block[1 + index / 2];
                let unsigned = if index & 1 == 1 {
                    packed >> 4
                } else {
                    packed & 15
                };
                let nibble = if unsigned >= 8 {
                    unsigned as i64 - 16
                } else {
                    unsigned as i64
                };
                let scaled = scale_nibble(nibble, range);
                let accumulator =
                    scaled * 256 + c0 * self.history1 as i64 + c1 * self.history2 as i64;
                let clamped = accumulator
                    .div_euclid(256)
                    .clamp(i16::MIN as i64, i16::MAX as i64) as i16;
                self.residue[index] = clamped;
                self.history2 = self.history1;
                self.history1 = clamped;
            }

            self.encoded_offset += BYTES_PER_BLOCK;
            self.stream_samples_remaining -= block_samples;
            self.residue_offset = 0;
            self.residue_len = block_samples;
        }
        Ok(produced)
    }

    /// Decodes the entire accepted stream into a fresh vector.
    pub fn decode_to_end(&mut self) -> Result<Vec<i16>, EaxaError> {
        let mut all = Vec::new();
        let mut chunk = [0i16; SAMPLES_PER_BLOCK * 8];
        loop {
            let produced = self.decode(&mut chunk)?;
            if produced == 0 {
                return Ok(all);
            }
            all.extend_from_slice(&chunk[..produced]);
        }
    }

    /// Samples still promised by the accepted stream but not yet emitted.
    pub fn samples_remaining(&self) -> usize {
        self.stream_samples_remaining + self.residue_len - self.residue_offset
    }
}

/// Range-scales one signed nibble. Ranges above 12 floor-divide (matching the
/// first-party `FloorDivide` semantics, i.e. rounding toward negative
/// infinity).
fn scale_nibble(nibble: i64, range: u32) -> i64 {
    if range <= 12 {
        nibble << (12 - range)
    } else {
        nibble.div_euclid(1 << (range - 12))
    }
}
