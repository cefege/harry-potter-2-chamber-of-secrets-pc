//! Line-by-line port of
//! `HarryPotter2/Unreal/Core/Src/FEAXABlockDecoder.cpp`: allocation-free
//! mono EA-XA block decoding. [`Decoder`] mirrors `FEAXABlockDecoder`
//! field for field; `feed`/`decode`/`reset_state` mirror its methods.

#[derive(Clone, Copy)]
struct Coefficients {
    c0: i32,
    c1: i32,
}

// EA's extended-XA N=8 predictor table from pinned vgmstream commit
// 11508e91a069066d0f6526d0096568b46c354014. See
// ThirdParty/vgmstream/COPYING for the retained ISC-style notice.
const COEFFICIENTS: [Coefficients; 16] = [
    Coefficients { c0: 0, c1: 0 },
    Coefficients { c0: 240, c1: 0 },
    Coefficients { c0: 460, c1: -208 },
    Coefficients { c0: 392, c1: -220 },
    Coefficients { c0: 488, c1: -240 },
    Coefficients { c0: 328, c1: -208 },
    Coefficients { c0: 440, c1: -168 },
    Coefficients { c0: 420, c1: -188 },
    Coefficients { c0: 432, c1: -176 },
    Coefficients { c0: 240, c1: -16 },
    Coefficients { c0: 416, c1: -192 },
    Coefficients { c0: 424, c1: -160 },
    Coefficients { c0: 288, c1: -8 },
    Coefficients { c0: 436, c1: -188 },
    Coefficients { c0: 224, c1: -1 },
    Coefficients { c0: 272, c1: -16 },
];

fn floor_divide(value: i32, divisor: i32) -> i32 {
    if value >= 0 {
        value / divisor
    } else {
        -((((-(value as i64)) + divisor as i64 - 1) / divisor as i64) as i32)
    }
}

fn scale_nibble(nibble: i32, range: i32) -> i32 {
    if range <= 12 {
        nibble * (1 << (12 - range))
    } else {
        floor_divide(nibble, 1 << (range - 12))
    }
}

fn clamp_sample(sample: i32) -> i16 {
    if sample > i16::MAX as i32 {
        i16::MAX
    } else if sample < i16::MIN as i32 {
        i16::MIN
    } else {
        sample as i16
    }
}

/// Port of the C++ `FEAXABlockDecoder` class. `encoded_data` is a borrowed,
/// non-owning pointer exactly as in the C++ (`Feed` stores it; the caller
/// must keep the buffer alive until the matching `decode` calls finish) —
/// the same contract the original class documents, not new unsafety.
pub struct Decoder {
    encoded_data: *const u8,
    encoded_bytes: i32,
    encoded_offset: i32,
    samples_remaining: i32,
    history1: i16,
    history2: i16,
    residue: [i16; 28],
    residue_offset: i32,
    residue_samples: i32,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    /// Port of the `FEAXABlockDecoder()` constructor.
    pub fn new() -> Self {
        Decoder {
            encoded_data: std::ptr::null(),
            encoded_bytes: 0,
            encoded_offset: 0,
            samples_remaining: 0,
            history1: 0,
            history2: 0,
            residue: [0; 28],
            residue_offset: 0,
            residue_samples: 0,
        }
    }

    /// Port of `Feed`.
    ///
    /// # Safety
    /// `data` must be valid for reads of `bytes` bytes for as long as this
    /// decoder subsequently calls `decode`, until the next `feed` (or the
    /// decoder is dropped) replaces it.
    pub unsafe fn feed(&mut self, data: *const u8, bytes: i32, num_samples: i32) -> bool {
        if bytes < 0 || num_samples < 0 || bytes % 15 != 0 {
            return false;
        }
        if bytes > 0 && data.is_null() {
            return false;
        }
        let capacity = (bytes as i64 / 15) * 28;
        if num_samples as i64 > capacity {
            return false;
        }
        self.encoded_data = data;
        self.encoded_bytes = bytes;
        self.encoded_offset = 0;
        self.samples_remaining = num_samples;
        self.residue_offset = 0;
        self.residue_samples = 0;
        true
    }

    /// Port of `Decode`.
    ///
    /// # Safety
    /// `dest` must be valid for writes of `samples` `i16`s. The buffer
    /// supplied to the most recent `feed` must still be valid for reads of
    /// its given length.
    pub unsafe fn decode(&mut self, dest: *mut i16, samples: i32) -> i32 {
        if dest.is_null() || samples <= 0 {
            return 0;
        }
        let mut produced = 0i32;
        while produced < samples {
            if self.residue_samples > 0 {
                let copy_samples = (samples - produced).min(self.residue_samples);
                for index in 0..copy_samples {
                    let value = self.residue[(self.residue_offset + index) as usize];
                    unsafe { *dest.offset((produced + index) as isize) = value };
                }
                produced += copy_samples;
                self.residue_offset += copy_samples;
                self.residue_samples -= copy_samples;
                continue;
            }
            if self.samples_remaining <= 0 || self.encoded_offset > self.encoded_bytes - 15 {
                break;
            }
            // SAFETY: feed() required `bytes % 15 == 0` and a buffer valid
            // for `bytes` reads; the loop guard above keeps encoded_offset
            // within [0, encoded_bytes - 15], so this block is in bounds.
            let block = unsafe {
                std::slice::from_raw_parts(
                    self.encoded_data.offset(self.encoded_offset as isize),
                    15,
                )
            };
            let predictor = (block[0] >> 4) as usize;
            let range = (block[0] & 15) as i32;
            let coefficients = COEFFICIENTS[predictor];
            let block_samples = self.samples_remaining.min(28);
            for index in 0..block_samples {
                let packed = block[1 + (index / 2) as usize];
                let unsigned_nibble = (if index & 1 != 0 {
                    packed >> 4
                } else {
                    packed & 15
                }) as i32;
                let signed_nibble = if unsigned_nibble >= 8 {
                    unsigned_nibble - 16
                } else {
                    unsigned_nibble
                };
                let sample = scale_nibble(signed_nibble, range);
                let accumulator = sample as i64 * 256
                    + coefficients.c0 as i64 * self.history1 as i64
                    + coefficients.c1 as i64 * self.history2 as i64;
                let decoded = if accumulator >= 0 {
                    (accumulator / 256) as i32
                } else {
                    -(((-accumulator + 255) / 256) as i32)
                };
                let clamped = clamp_sample(decoded);
                self.residue[index as usize] = clamped;
                self.history2 = self.history1;
                self.history1 = clamped;
            }
            self.encoded_offset += 15;
            self.samples_remaining -= block_samples;
            self.residue_offset = 0;
            self.residue_samples = block_samples;
        }
        produced
    }

    /// Port of `ResetState`.
    pub fn reset_state(&mut self, sample1: i16, sample2: i16) {
        self.history1 = sample1;
        self.history2 = sample2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_block(predictor: u8, range: u8, nibble: i8) -> [u8; 15] {
        let mut block = [0u8; 15];
        block[0] = (predictor << 4) | (range & 15);
        let packed = (nibble as u8) & 15;
        let byte = packed | (packed << 4);
        for b in block.iter_mut().skip(1) {
            *b = byte;
        }
        block
    }

    #[test]
    fn zero_predictor_passes_samples_through_scaled() {
        // Predictor 0 has C0=C1=0, so output is just the scaled nibble.
        let block = make_block(0, 12, 3); // range<=12: scale factor 1<<(12-12)=1
        let mut decoder = Decoder::new();
        let mut out = [0i16; 28];
        unsafe {
            assert!(decoder.feed(block.as_ptr(), 15, 28));
            assert_eq!(decoder.decode(out.as_mut_ptr(), 28), 28);
        }
        assert!(out.iter().all(|&s| s == 3), "{out:?}");
    }

    #[test]
    fn feed_rejects_non_multiple_of_15() {
        let block = [0u8; 14];
        let mut decoder = Decoder::new();
        unsafe {
            assert!(!decoder.feed(block.as_ptr(), 14, 1));
        }
    }

    #[test]
    fn feed_rejects_oversized_sample_request() {
        let block = make_block(0, 12, 0);
        let mut decoder = Decoder::new();
        unsafe {
            assert!(!decoder.feed(block.as_ptr(), 15, 29));
            assert!(decoder.feed(block.as_ptr(), 15, 28));
        }
    }

    #[test]
    fn decode_splits_across_partial_requests() {
        let block = make_block(0, 12, 5);
        let mut decoder = Decoder::new();
        let mut out = [0i16; 10];
        unsafe {
            assert!(decoder.feed(block.as_ptr(), 15, 10));
            assert_eq!(decoder.decode(out.as_mut_ptr(), 3), 3);
            assert_eq!(decoder.decode(out.as_mut_ptr().add(3), 7), 7);
        }
        assert!(out.iter().all(|&s| s == 5), "{out:?}");
    }

    #[test]
    fn reset_state_seeds_history() {
        let mut decoder = Decoder::new();
        decoder.reset_state(1000, -1000);
        // Predictor 1 has C0=240, C1=0: first sample mixes in History1.
        let block = make_block(1, 12, 0);
        let mut out = [0i16; 1];
        unsafe {
            assert!(decoder.feed(block.as_ptr(), 15, 1));
            decoder.decode(out.as_mut_ptr(), 1);
        }
        // accumulator = 0*256 + 240*1000 + 0*(-1000) = 240000; /256 = 937
        assert_eq!(out[0], 937);
    }
}
