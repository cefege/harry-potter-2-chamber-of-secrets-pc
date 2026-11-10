//! A small non-cryptographic hasher for engine-thread tables keyed by pointers, indices and
//! short names. `std`'s default SipHash showed up at about 13% of a save-game profile; these
//! tables are never exposed to attacker-chosen keys, so the cheaper multiplicative hash is
//! enough. Iteration order is not part of any contract (it was already random per process).

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// FxHash-style word hasher with a final rotate so aligned pointer keys do not leave the low
/// bits (which index the table) constant.
#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);

impl FastHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            self.add(u64::from_le_bytes(chunk.try_into().expect("8-byte chunk")));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word) ^ ((rest.len() as u64) << 56));
        }
    }
    #[inline]
    fn write_u8(&mut self, value: u8) { self.add(u64::from(value)); }
    #[inline]
    fn write_u16(&mut self, value: u16) { self.add(u64::from(value)); }
    #[inline]
    fn write_u32(&mut self, value: u32) { self.add(u64::from(value)); }
    #[inline]
    fn write_u64(&mut self, value: u64) { self.add(value); }
    #[inline]
    fn write_usize(&mut self, value: usize) { self.add(value as u64); }
    #[inline]
    fn finish(&self) -> u64 { self.0.rotate_left(26) }
}

pub type FastBuildHasher = BuildHasherDefault<FastHasher>;
pub type FastMap<K, V> = HashMap<K, V, FastBuildHasher>;
pub type FastSet<K> = HashSet<K, FastBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_pointer_keys_spread_over_the_low_bits() {
        let mut buckets = [0u32; 64];
        for index in 0..4096usize {
            let mut hasher = FastHasher::default();
            hasher.write_usize(0x1000_0000 + index * 64);
            buckets[(hasher.finish() & 63) as usize] += 1;
        }
        // 4096 keys over 64 buckets: a good spread keeps every bucket near 64.
        assert!(buckets.iter().all(|&count| (16..=160).contains(&count)), "{buckets:?}");
    }

    #[test]
    fn maps_behave_like_std_maps() {
        let mut map: FastMap<String, i32> = FastMap::default();
        map.insert("a".into(), 1);
        map.insert("b".into(), 2);
        assert_eq!(map.get("a"), Some(&1));
        assert_eq!(map.remove("b"), Some(2));
        assert!(map.get("b").is_none());
    }
}
