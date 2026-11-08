//! `FOutputDeviceFile::WriteRaw` from `Core/Inc/FOutputDeviceFile.h`: host `TCHAR`
//! text to UTF-16LE, batched into the original 512-byte `Encoded` buffer and
//! flushed through `LogAr->Serialize` ([`Hp2OutputDeviceFileServices::serialize`])
//! at the same cadence: before an append that would overflow the buffer, plus a
//! final flush for any remainder. The `Serialize` call itself stays a C++ leaf
//! (stateful virtual dispatch on a C++-owned `FArchive*`); everything else —
//! the surrogate-pair split, U+FFFD substitution, and buffer bookkeeping — is
//! pure computation and lives here.
//!
//! Divergence: the original also has an `if( sizeof(TCHAR)==sizeof(UNICHAR) )`
//! branch that copies `ToUnicode(*C++)` directly, one unit per `TCHAR`. This
//! build's `TCHAR` is the host `wchar_t`, 32 bits (`HP2_HOST_WCHAR_TCHAR`,
//! `Core/Inc/UnGnuG.h`), never the same size as the 16-bit `UNICHAR`, so that
//! branch is dead code and is not ported — the same call `hp2-engine`'s
//! `stat_log::encode_utf16le` makes for the analogous branch in `AStatLog.cpp`.

use crate::ansi::Tchar;
use std::ffi::c_void;

/// `ARRAY_COUNT(Encoded)`: the original's local `BYTE Encoded[512]`.
const ENCODED_BUFFER_LEN: usize = 512;

/// C++ services (`Hp2OutputDeviceFileServices`). `serialize` is `LogAr->Serialize(V, Length)`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2OutputDeviceFileServices {
    pub serialize: unsafe extern "C" fn(*mut c_void, *const u8, i32) -> i32,
}

/// One host code point's UTF-16 units: `CodePoint = HostChar` when it is in
/// `[0, 0x10ffff]`, else U+FFFD; a code point already in the surrogate range
/// `[0xd800, 0xdfff]` also becomes U+FFFD; a code point above U+FFFF splits
/// into a high/low surrogate pair. Returns the unit count (1 or 2).
fn encode_code_point(c: Tchar, units: &mut [u16; 2]) -> usize {
    let raw = c as i64;
    let mut code = if (0..=0x10ffff).contains(&raw) { raw as u32 } else { 0xfffd };
    if (0xd800..=0xdfff).contains(&code) {
        code = 0xfffd;
    }
    if code > 0xffff {
        let v = code - 0x10000;
        units[0] = (0xd800 + (v >> 10)) as u16;
        units[1] = (0xdc00 + (v & 0x3ff)) as u16;
        2
    } else {
        units[0] = code as u16;
        1
    }
}

/// Encodes `text` (host code points, no terminator) to UTF-16LE, batching into
/// a 512-byte local buffer and calling `flush` with each full buffer, plus once
/// more at the end for any remainder. Matches the original's flush cadence: a
/// unit (or pair) that would overflow the buffer flushes the buffer first, then
/// starts the next one empty.
fn encode_buffered(
    text: &[Tchar],
    mut flush: impl FnMut(&[u8]) -> Result<(), i32>,
) -> Result<(), i32> {
    let mut encoded = [0u8; ENCODED_BUFFER_LEN];
    let mut byte_count = 0usize;
    for &c in text {
        let mut units = [0u16; 2];
        let unit_count = encode_code_point(c, &mut units);
        if byte_count + unit_count * 2 > ENCODED_BUFFER_LEN {
            flush(&encoded[..byte_count])?;
            byte_count = 0;
        }
        for &unit in &units[..unit_count] {
            let bytes = unit.to_le_bytes();
            encoded[byte_count] = bytes[0];
            encoded[byte_count + 1] = bytes[1];
            byte_count += 2;
        }
    }
    if byte_count > 0 {
        flush(&encoded[..byte_count])?;
    }
    Ok(())
}

/// `FOutputDeviceFile::WriteRaw(const TCHAR* C)`'s `#if UNICODE` body.
///
/// # Safety
/// `text` must be NUL-terminated and readable for its length. `archive` must be
/// the live `FArchive*` (`LogAr`) that `services.serialize` forwards to.
pub unsafe fn write_raw(
    text: *const Tchar,
    archive: *mut c_void,
    services: &Hp2OutputDeviceFileServices,
) -> Result<(), i32> {
    let mut len = 0usize;
    unsafe {
        while *text.add(len) != 0 {
            len += 1;
        }
    }
    let units = unsafe { std::slice::from_raw_parts(text, len) };
    encode_buffered(units, |chunk| {
        let status = unsafe { (services.serialize)(archive, chunk.as_ptr(), chunk.len() as i32) };
        if status == 0 { Ok(()) } else { Err(status) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &[Tchar]) -> Vec<Vec<u8>> {
        let mut flushes = Vec::new();
        encode_buffered(text, |chunk| {
            flushes.push(chunk.to_vec());
            Ok(())
        })
        .unwrap();
        flushes
    }

    #[test]
    fn ascii_only() {
        let text: [Tchar; 2] = [b'A' as Tchar, b'B' as Tchar];
        let flushes = run(&text);
        assert_eq!(flushes, vec![vec![0x41, 0x00, 0x42, 0x00]]);
    }

    #[test]
    fn supplementary_code_point_splits_into_surrogate_pair() {
        // U+1F600 GRINNING FACE: v = 0x1F600 - 0x10000 = 0xF600.
        // high = 0xd800 + (0xF600 >> 10) = 0xd83d, low = 0xdc00 + (0xF600 & 0x3ff) = 0xde00.
        let text: [Tchar; 1] = [0x1F600];
        let flushes = run(&text);
        assert_eq!(flushes, vec![vec![0x3d, 0xd8, 0x00, 0xde]]);
    }

    #[test]
    fn surrogate_range_code_point_becomes_replacement_character() {
        for bad in [0xd800 as Tchar, 0xdfff as Tchar, -1 as Tchar, 0x110000 as Tchar] {
            let flushes = run(&[bad]);
            assert_eq!(flushes, vec![vec![0xfd, 0xff]], "input {bad:#x}");
        }
    }

    #[test]
    fn long_string_flushes_at_512_byte_boundary() {
        // 300 ASCII units (2 bytes each): the 257th unit would push byte_count
        // from 512 to 514, so it flushes the first 256 units (512 bytes) before
        // appending, then the remaining 44 units (88 bytes) flush at the end.
        let text: Vec<Tchar> = std::iter::repeat(b'A' as Tchar).take(300).collect();
        let flushes = run(&text);
        assert_eq!(flushes.len(), 2);
        assert_eq!(flushes[0].len(), 512);
        assert_eq!(flushes[1].len(), 88);
        assert!(flushes[0].iter().copied().eq(std::iter::repeat([0x41, 0x00]).take(256).flatten()));
        assert!(flushes[1].iter().copied().eq(std::iter::repeat([0x41, 0x00]).take(44).flatten()));
    }

    #[test]
    fn empty_string_emits_nothing() {
        assert!(run(&[]).is_empty());
    }
}
