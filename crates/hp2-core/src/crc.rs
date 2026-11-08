//! CRC helpers from `HarryPotter2/Unreal/Core/Src/UnMisc.cpp`.
//!
//! `GCRCTable` remains C++-owned ABI storage because `appStrihash` and the
//! Core name service read it directly; `appInit` fills it from this module.

use crate::ansi::Tchar;

const POLY: u32 = 0x04c1_1db7;

const fn build_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = (index as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 0x8000_0000 != 0 {
                (value << 1) ^ POLY
            } else {
                value << 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

/// CRC-32 lookup table, initialized at compile time.
pub static CRC_TABLE: [u32; 256] = build_table();

/// Copy the Rust-generated table into the legacy exported `GCRCTable` buffer.
///
/// # Safety
/// `out` must point to 256 writable `u32` entries.
pub unsafe fn fill_table(out: *mut u32) {
    for (index, value) in CRC_TABLE.iter().copied().enumerate() {
        unsafe { out.add(index).write(value) };
    }
}

#[inline]
fn update(crc: u32, byte: u8) -> u32 {
    (crc << 8) ^ CRC_TABLE[((crc >> 24) as u8 ^ byte) as usize]
}

/// `appMemCrc`, preserving the caller-provided seed and empty/negative lengths.
///
/// # Safety
/// If `length` is positive, `data` must be readable for `length` bytes.
pub unsafe fn mem_crc(data: *const u8, length: i32, seed: u32) -> u32 {
    let mut crc = !seed;
    for offset in 0..length.max(0) as usize {
        crc = update(crc, unsafe { *data.add(offset) });
    }
    !crc
}

/// `appStrCrc` / `appStrCrcCaps` for supported `UNICODE` builds.
///
/// Each TCHAR contributes its low byte and then the next byte, matching the
/// original two-byte `UNICODE` CRC representation even though host TCHAR is
/// 32-bit. Uppercasing is the legacy ASCII-only `appToUpper` operation.
///
/// # Safety
/// `data` must point to a readable NUL-terminated TCHAR string.
pub unsafe fn string_crc(data: *const Tchar, uppercase: bool) -> u32 {
    let mut crc = u32::MAX;
    let mut cursor = data;
    loop {
        let mut character = unsafe { *cursor };
        if character == 0 {
            return !crc;
        }
        if uppercase && character >= b'a' as Tchar && character <= b'z' as Tchar {
            character -= (b'a' - b'A') as Tchar;
        }
        let value = character as u32;
        crc = update(crc, value as u8);
        #[cfg(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "x86_64")
        ))]
        let high = ((character as i32 >> 8) & 0xff) as u8;
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let high = ((value >> 8) & 0xff) as u8;
        crc = update(crc, high);
        cursor = unsafe { cursor.add(1) };
    }
}

#[cfg(test)]
mod tests {
    use super::{CRC_TABLE, fill_table, mem_crc, string_crc};
    use crate::ansi::Tchar;
    use std::ptr::null;

    fn update_reference(mut crc: u32, byte: u8) -> u32 {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
        crc
    }

    fn string_reference(input: &[u32], uppercase: bool) -> u32 {
        let mut crc = u32::MAX;
        for &character in input {
            if character == 0 {
                break;
            }
            let character = if uppercase && (b'a' as u32..=b'z' as u32).contains(&character) {
                character - (b'a' - b'A') as u32
            } else {
                character
            };
            crc = update_reference(crc, character as u8);
            crc = update_reference(crc, (character >> 8) as u8);
        }
        !crc
    }

    #[test]
    fn table_and_memory_crc_match_independent_bitwise_oracle() {
        assert_eq!(CRC_TABLE[0], 0);
        assert_eq!(CRC_TABLE[1], 0x04c1_1db7);
        let bytes = b"123456789";
        // SAFETY: `bytes` is readable for nine bytes.
        let whole = unsafe { mem_crc(bytes.as_ptr(), bytes.len() as i32, 0) };
        assert_eq!(whole, 0xfc89_1918);
        let expected = bytes.iter().copied().fold(u32::MAX, update_reference);
        assert_eq!(whole, !expected);

        // Streaming CRC calls must chain exactly like a single call.
        let first = unsafe { mem_crc(bytes.as_ptr(), 4, 0) };
        let chained = unsafe { mem_crc(bytes.as_ptr().add(4), 5, first) };
        assert_eq!(chained, whole);
        // SAFETY: non-positive lengths do not read `data`.
        assert_eq!(unsafe { mem_crc(null(), 0, 0x1234_5678) }, 0x1234_5678);
        assert_eq!(unsafe { mem_crc(null(), -1, 0x8765_4321) }, 0x8765_4321);

        let mut copied = [0; 256];
        // SAFETY: `copied` has 256 writable entries.
        unsafe { fill_table(copied.as_mut_ptr()) };
        assert_eq!(copied, CRC_TABLE);
    }

    #[test]
    fn string_crcs_match_bitwise_oracle_for_ascii_and_wide_characters() {
        for input in [
            vec![b'a' as u32, b'Z' as u32, 0],
            vec![0x1234, b'x' as u32, 0],
            vec![0x1f600, 0],
            vec![0],
        ] {
            let wide: Vec<Tchar> = input.iter().map(|&value| value as Tchar).collect();
            // SAFETY: `wide` is NUL-terminated and remains alive for both calls.
            let actual = unsafe { string_crc(wide.as_ptr(), false) };
            let actual_caps = unsafe { string_crc(wide.as_ptr(), true) };
            assert_eq!(actual, string_reference(&input, false));
            assert_eq!(actual_caps, string_reference(&input, true));
        }
    }
}
