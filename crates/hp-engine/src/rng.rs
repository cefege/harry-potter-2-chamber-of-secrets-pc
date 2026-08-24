//! Seeded determinism RNG policy (Docs/RNG_TICK_DIVERGENCE.md).
//!
//! The C++ engine drives gameplay randomness from unseeded libc `rand()`
//! over variable-delta ticks, which makes cutscene and scripted-cue framing
//! non-reproducible. This engine instead owns ONE
//! `rand_chacha::ChaCha8Rng` per process, seeded from `--rng-seed`
//! (default literal `0x48503200`), consumed in a fixed order every tick.

use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

/// Default process seed (`HP2` + NUL padding, spelled `0x48503200`).
pub const DEFAULT_SEED: u64 = 0x4850_3200;

/// The single per-process random stream.
pub struct SimRng {
    inner: ChaCha8Rng,
}

impl SimRng {
    pub fn seeded(seed: u64) -> Self {
        Self {
            inner: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Uniform `f32` in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        // 24 bits of mantissa keeps the value exact and the stream
        // platform-independent.
        ((self.inner.next_u32() >> 8) as f32) / (1u32 << 24) as f32
    }

    pub fn next_u32(&mut self) -> u32 {
        self.inner.next_u32()
    }

    pub fn fill_bytes(&mut self, out: &mut [u8]) {
        self.inner.fill_bytes(out);
    }
}

impl Default for SimRng {
    fn default() -> Self {
        Self::seeded(DEFAULT_SEED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_stream() {
        let mut a = SimRng::seeded(0x4850_3200);
        let mut b = SimRng::seeded(0x4850_3200);
        for _ in 0..64 {
            assert_eq!(a.next_f32(), b.next_f32());
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn range_and_distinctness() {
        let mut rng = SimRng::seeded(1);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..256 {
            let v = rng.next_f32();
            assert!((0.0..1.0).contains(&v));
            seen.insert(v.to_bits());
        }
        assert!(seen.len() > 200);
    }
}
