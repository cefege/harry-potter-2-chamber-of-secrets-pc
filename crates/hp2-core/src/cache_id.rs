//! `Core/Inc/UnCId.h` cache-ID construction.
//!
//! C++ obtains `UObject::GetIndex()` through the existing object ABI. These
//! functions preserve the legacy integer-promotion and 32-bit wrapping rules.

#[inline]
pub fn frame(base: u32, frame_index: u32) -> u64 {
    u64::from(base) + (u64::from(frame_index) << 8)
}

#[inline]
pub fn object(base: u32, object_index: u32, frame_index: u32) -> u64 {
    u64::from(base.wrapping_add(object_index.wrapping_shl(8))) + (u64::from(frame_index) << 32)
}

#[inline]
pub fn word(base: u32, word: u32, byte: u32, frame_index: u32) -> u64 {
    let low = base
        .wrapping_add(byte.wrapping_shl(8))
        .wrapping_add(word.wrapping_shl(16));
    u64::from(low) + (u64::from(frame_index) << 32)
}

#[inline]
pub fn bytes(base: u32, byte_a: u32, byte_b: u32, byte_c: u32, frame_index: u32) -> u64 {
    let low = base
        .wrapping_add(byte_a.wrapping_shl(8))
        .wrapping_add(byte_b.wrapping_shl(16))
        .wrapping_add(byte_c.wrapping_shl(24));
    u64::from(low) + (u64::from(frame_index) << 32)
}

#[inline]
pub fn value(base: u32, value: u64) -> u64 {
    u64::from(base) + (value & !u64::from(0xff_u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_pack_legacy_fields() {
        assert_eq!(frame(0x15, 0x12_3456), 0x12_3456_15);
        assert_eq!(bytes(0x15, 0x12, 0x34, 0x56, 0x789a), 0x789a_5634_1215);
        assert_eq!(word(0x18, 0x1234, 0x56, 0x789a), 0x789a_1234_5618);
        assert_eq!(value(0x1f, 0x1122_3344_5566_77aa), 0x1122_3344_5566_771f);
    }

    #[test]
    fn low_32_bit_intermediate_additions_wrap_before_frame_is_added() {
        assert_eq!(object(0xff, u32::MAX, 1), 0x0000_0001_ffff_ffff);
        assert_eq!(word(0xff, u32::MAX, u32::MAX, 1), 0x0000_0001_fffe_ffff);
        assert_eq!(
            bytes(0xff, u32::MAX, u32::MAX, u32::MAX, 1),
            0x0000_0001_fefe_ffff
        );
    }
}
