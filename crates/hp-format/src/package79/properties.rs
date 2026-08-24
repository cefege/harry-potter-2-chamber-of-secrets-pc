//! Tagged property grammar: name index, Info byte (type = low nibble, 0x70
//! size code, 0x80 array flag), struct-name index where applicable, size,
//! optional array index, raw payload.
//!
//! Grammar cross-reference: `Build/smoke_maps.py::_tagged_properties`; the
//! payload bytes are kept verbatim so re-encoding is byte-exact regardless of
//! type-specific interpretation.

use super::cursor::{ByteCursor, read_compact_index, write_compact_index};
use super::error::{PackageError, PackageResult, layout};
use super::tables::NameEntry;

/// Property kind byte for `Bool` (value carried in the array-flag bit).
pub const PROPERTY_TYPE_BOOL: u8 = 3;
/// Property kind byte for `Struct` (carries a trailing struct-name index).
pub const PROPERTY_TYPE_STRUCT: u8 = 10;
/// Property kind byte for `String`.
pub const PROPERTY_TYPE_STR: u8 = 13;

/// One tagged property. `size_code` and `array_flag` preserve the exact
/// Info-byte bits; `array_index` is decoded only when the flag is set and the
/// kind is not `Bool`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyTag {
    pub name_index: i32,
    /// Low nibble of the Info byte.
    pub kind: u8,
    /// Mid nibble of the Info byte (`0x00..=0x70` in steps of `0x10`); the
    /// implicit sizes are `{0x00:1, 0x10:2, 0x20:4, 0x30:12, 0x40:16}` and
    /// `{0x50:u8, 0x60:u16, 0x70:i32}` prefix the payload with an explicit
    /// size.
    pub size_code: u8,
    /// Raw 0x80 bit of the Info byte.
    pub array_flag: bool,
    /// Struct-name index, present exactly when `kind == PROPERTY_TYPE_STRUCT`.
    pub struct_name_index: Option<i32>,
    /// Element index for array-flagged tags of non-Bool kinds.
    pub array_index: Option<i32>,
    /// Raw payload bytes (`size` long).
    pub payload: Vec<u8>,
}

impl PropertyTag {
    fn info_byte(&self) -> u8 {
        self.kind | self.size_code | if self.array_flag { 0x80 } else { 0 }
    }
}

fn read_size(cur: &mut ByteCursor<'_>, size_code: u8) -> PackageResult<usize> {
    match size_code {
        0x00 => Ok(1),
        0x10 => Ok(2),
        0x20 => Ok(4),
        0x30 => Ok(12),
        0x40 => Ok(16),
        0x50 => Ok(usize::from(cur.u8()?)),
        0x60 => Ok(usize::from(cur.u16()?)),
        _ => {
            let size = cur.i32()?;
            if size < 0 {
                return Err(layout(format!("negative property size {size}")));
            }
            Ok(size as usize)
        }
    }
}

/// Read a tagged-property list from `cur` until the terminator whose name text
/// is `"None"` (the terminator's compact index is consumed but not returned).
///
/// Name indices are validated against `names`; payloads are copied verbatim.
/// Trailing bytes after the terminator belong to whatever follows the property
/// region and are left unread.
pub fn read_property_tags(
    cur: &mut ByteCursor<'_>,
    names: &[NameEntry],
) -> PackageResult<Vec<PropertyTag>> {
    let mut properties = Vec::new();
    loop {
        let name_index = read_compact_index(cur)?;
        if name_index < 0 || name_index as usize >= names.len() {
            return Err(PackageError::BadNameIndex {
                index: name_index,
                count: names.len(),
            });
        }
        if names[name_index as usize].text == "None" {
            return Ok(properties);
        }

        let info = cur.u8()?;
        let kind = info & 0x0F;
        let size_code = info & 0x70;
        let array_flag = info & 0x80 != 0;

        let struct_name_index = if kind == PROPERTY_TYPE_STRUCT {
            let index = read_compact_index(cur)?;
            if index < 0 || index as usize >= names.len() {
                return Err(PackageError::BadNameIndex {
                    index,
                    count: names.len(),
                });
            }
            Some(index)
        } else {
            None
        };

        let size = read_size(cur, size_code)?;
        let array_index = if array_flag && kind != PROPERTY_TYPE_BOOL {
            Some(read_compact_index(cur)?)
        } else {
            None
        };
        let payload = cur.take(size)?.to_vec();

        properties.push(PropertyTag {
            name_index,
            kind,
            size_code,
            array_flag,
            struct_name_index,
            array_index,
            payload,
        });
    }
}

/// Serialize one property tag.
pub fn write_property_tag(out: &mut Vec<u8>, tag: &PropertyTag) {
    write_compact_index(out, tag.name_index);
    out.push(tag.info_byte());
    if let Some(index) = tag.struct_name_index {
        write_compact_index(out, index);
    }
    match tag.size_code {
        0x50 => out.push(tag.payload.len() as u8),
        0x60 => out.extend_from_slice(&(tag.payload.len() as u16).to_le_bytes()),
        0x70 => out.extend_from_slice(&(tag.payload.len() as i32).to_le_bytes()),
        _ => {}
    }
    if let Some(index) = tag.array_index {
        write_compact_index(out, index);
    }
    out.extend_from_slice(&tag.payload);
}

/// Close a property list by emitting the compact index of the `"None"` name
/// entry.
pub fn write_property_terminator(out: &mut Vec<u8>, none_name_index: i32) {
    write_compact_index(out, none_name_index);
}
