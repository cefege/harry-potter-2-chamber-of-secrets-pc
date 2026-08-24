//! Position-tracking little-endian byte cursor plus the compact-index and
//! FString primitives.

use super::error::{PackageError, PackageResult, layout};

/// Unreal package tag shared by every version this module accepts.
pub const PACKAGE_TAG: u32 = 0x9E2A83C1;
/// Inclusive FileVersion window accepted by the reader.
pub const PACKAGE_MIN_VERSION: i32 = 60;
/// Inclusive FileVersion window accepted by the reader.
pub const PACKAGE_MAX_VERSION: i32 = 79;
/// LicenseeVersion this reader accepts.
pub const LICENSEE_VERSION: i32 = 0;
/// Upper bound on serialized name length, in character units.
pub const MAX_NAME_UNITS: usize = 128;

/// Position-tracking little-endian cursor over a borrowed byte slice.
#[derive(Debug)]
pub struct ByteCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    /// Cursor starting at offset 0 of `data`.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Cursor starting at `pos` of `data`.
    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    /// Current read offset.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Move the read offset (no bounds check; later reads report truncation).
    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Total length of the underlying slice.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// True when the underlying slice is empty (clippy::len_without_is_empty).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// True when the cursor sits at the end of the input.
    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn require(&self, need: usize) -> PackageResult<()> {
        if self
            .pos
            .checked_add(need)
            .is_none_or(|end| end > self.data.len())
        {
            return Err(PackageError::Truncated {
                need,
                got: self.data.len(),
            });
        }
        Ok(())
    }

    /// Borrow the next `need` bytes and advance.
    pub fn take(&mut self, need: usize) -> PackageResult<&'a [u8]> {
        self.require(need)?;
        let start = self.pos;
        self.pos += need;
        Ok(&self.data[start..self.pos])
    }

    /// Read one unsigned byte.
    pub fn u8(&mut self) -> PackageResult<u8> {
        Ok(self.take(1)?[0])
    }

    /// Read one little-endian `u16`.
    pub fn u16(&mut self) -> PackageResult<u16> {
        let raw = self.take(2)?;
        Ok(u16::from_le_bytes([raw[0], raw[1]]))
    }

    /// Read one little-endian `u32`.
    pub fn u32(&mut self) -> PackageResult<u32> {
        let raw = self.take(4)?;
        Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    /// Read one little-endian `i32`.
    pub fn i32(&mut self) -> PackageResult<std::primitive::i32> {
        Ok(self.u32()? as _)
    }
}

/// Decode Latin-1 bytes into their exact Unicode equivalent (ANSI FString
/// payloads use this mapping).
pub fn latin1_str(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Encode a signed compact index in its unique shortest form.
///
/// Every int32 value encodes to at most five bytes, so this cannot fail.
pub fn write_compact_index(out: &mut Vec<u8>, value: std::primitive::i32) {
    let negative = value < 0;
    // Magnitude of INT32_MIN fits in u32/i64 without overflow.
    let mut magnitude = (value as i64).unsigned_abs();

    let mut first = magnitude as u8 & 0x3F;
    magnitude >>= 6;
    first |= u8::from(negative) << 7;
    if magnitude != 0 {
        first |= 0x40;
    }
    out.push(first);

    while magnitude != 0 {
        let payload = magnitude as u8 & 0x7F;
        magnitude >>= 7;
        if out.len() == 4 {
            out.push(payload);
            debug_assert_eq!(
                magnitude, 0,
                "int32 compact index needs more than five bytes"
            );
            break;
        }
        out.push(payload | (u8::from(magnitude != 0) * 0x80));
    }
}

/// Serialize a signed compact index to a fresh vector (test convenience).
#[doc(hidden)]
pub fn encode_compact_index(value: std::primitive::i32) -> Vec<u8> {
    let mut out = Vec::new();
    write_compact_index(&mut out, value);
    out
}

/// Read a signed compact index, rejecting non-shortest and out-of-range
/// encodings exactly like the reference decoder.
pub fn read_compact_index(cur: &mut ByteCursor<'_>) -> PackageResult<std::primitive::i32> {
    let start = cur.position();
    let mut byte = cur.u8()?;
    // First byte carries the low six magnitude bits (python reference:
    // `magnitude = first & 0x3F`) — omitting them corrupted every decode.
    let mut magnitude: i64 = i64::from(byte & 0x3F);
    let mut shift: u32 = 6;
    let mut byte_count = 1usize;
    while byte & if byte_count == 1 { 0x40 } else { 0x80 } != 0 {
        if byte_count == 5 {
            return Err(PackageError::BadCompactIndex);
        }
        byte = cur.u8()?;
        byte_count += 1;
        magnitude |= i64::from(byte & 0x7F) << shift;
        shift += 7;
        if byte_count == 5 {
            break;
        }
    }

    let value = if cur.data()[start] & 0x80 != 0 {
        -magnitude
    } else {
        magnitude
    };
    if !(-(1i64 << 31)..=(1i64 << 31) - 1).contains(&value) {
        return Err(PackageError::CompactIndexOutOfRange);
    }
    let encoded = &cur.data()[start..cur.position()];
    if encoded != encode_compact_index(value as std::primitive::i32) {
        return Err(PackageError::NonCanonicalCompactIndex);
    }
    Ok(value as std::primitive::i32)
}

impl<'a> ByteCursor<'a> {
    /// Borrowed view of the whole backing slice (for canonicality checks).
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

/// Read a serialized FString: signed compact-index unit count (negative =
/// UTF-16LE), NUL-terminated payload. ANSI payloads decode as Latin-1;
/// malformed UTF-16 sequences decode with U+FFFD replacements, matching the
/// engine's tolerant loader. Truncation and missing terminators are loud
/// errors.
pub fn read_fstring(cur: &mut ByteCursor<'_>) -> PackageResult<String> {
    let count = read_compact_index(cur)?;
    let units = count.unsigned_abs() as usize;
    if units == 0 {
        return Ok(String::new());
    }
    if count > 0 {
        let raw = cur.take(units)?;
        if raw[raw.len() - 1] != 0 {
            return Err(PackageError::BadString);
        }
        Ok(latin1_str(&raw[..raw.len() - 1]))
    } else {
        let raw = cur.take(
            units
                .checked_mul(2)
                .ok_or_else(|| layout("UTF-16 FString size overflow"))?,
        )?;
        if raw.len() < 2 || raw[raw.len() - 1] != 0 || raw[raw.len() - 2] != 0 {
            return Err(PackageError::BadString);
        }
        let units16: Vec<u16> = raw[..raw.len() - 2]
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect();
        let mut text = String::with_capacity(units16.len());
        let mut iter = units16.into_iter();
        while let Some(unit) = iter.next() {
            match unit {
                0xD800..=0xDBFF => {
                    let low = iter.next();
                    match low {
                        Some(0xDC00..=0xDFFF) => {
                            let high = (unit - 0xD800) as u32;
                            let low_bits = (low.unwrap() - 0xDC00) as u32;
                            text.push(char::from_u32(0x10000 + (high << 10) + low_bits).unwrap());
                        }
                        _ => text.push('\u{FFFD}'),
                    }
                }
                0xDC00..=0xDFFF => text.push('\u{FFFD}'),
                _ => text.push(char::from_u32(u32::from(unit)).expect("BMP scalar")),
            }
        }
        Ok(text)
    }
}

/// Serialize a FString using UTF-16LE when any code point exceeds U+00FF,
/// otherwise the compact ANSI form — mirroring which form the engine picks.
pub fn write_fstring(out: &mut Vec<u8>, text: &str) {
    if text.chars().any(|c| c as u32 > 0xFF) {
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);
        write_compact_index(out, -(units.len() as std::primitive::i32));
        for unit in units {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    } else {
        let bytes = latin1_bytes(text);
        write_compact_index(out, bytes.len() as std::primitive::i32 + 1);
        out.extend_from_slice(&bytes);
        out.push(0);
    }
}

fn latin1_bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| c as u32 as u8).collect()
}
