//! Protocol sequence arithmetic from `Engine/Inc/UnNet.h`.
//!
//! Inputs follow the Unreal network protocol: `max` is a power of two and
//! sequence numbers are within that range. The wrapping operations make the
//! fixed-width arithmetic explicit without expanding C++'s undefined
//! signed-overflow domain.

/// Return the signed distance from `reference` to `value` in a ring of `max`.
#[inline]
pub fn best_signed_difference(value: i32, reference: i32, max: i32) -> i32 {
    let offset = value.wrapping_sub(reference).wrapping_add(max / 2);
    (offset & (max - 1)).wrapping_sub(max / 2)
}

/// Normalize `value` to the sequence interval nearest `reference`.
#[inline]
pub fn make_relative(value: i32, reference: i32, max: i32) -> i32 {
    reference.wrapping_add(best_signed_difference(value, reference, max))
}

#[cfg(test)]
mod tests {
    use super::{best_signed_difference, make_relative};

    #[test]
    fn preserves_protocol_half_range_boundary() {
        assert_eq!(best_signed_difference(511, 0, 1024), 511);
        assert_eq!(best_signed_difference(512, 0, 1024), -512);
        assert_eq!(make_relative(0, 1023, 1024), 1024);
    }

    #[test]
    fn preserves_packet_and_legacy_packet_ranges() {
        assert_eq!(best_signed_difference(16_383, 0, 16_384), -1);
        assert_eq!(make_relative(0, 16_383, 16_384), 16_384);
        assert_eq!(best_signed_difference(65_535, 0, 65_536), -1);
        assert_eq!(make_relative(0, 65_535, 65_536), 65_536);
    }
}
