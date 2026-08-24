//! Name-table, import-table, and export-table entry structures with their
//! version-dependent parsers and serializers.
//!
//! Normative behavior source: `Build/package79_reference.py::parse_names`,
//! `parse_imports`, `parse_exports`.

use super::cursor::{
    ByteCursor, MAX_NAME_UNITS, latin1_str, read_compact_index, write_compact_index,
};
use super::error::{PackageError, PackageResult, layout};
use super::summary::PackageSummary;

/// Serialized FString encoding chosen for one name entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameEncoding {
    /// Positive unit count: Latin-1 bytes including the NUL terminator.
    Ansi,
    /// Negative unit count: UTF-16LE code units including the NUL terminator.
    Utf16,
}

/// One name-table entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameEntry {
    pub text: String,
    pub flags: u32,
    /// Original serialized form; preserved so re-encoding is byte-exact even
    /// when the text would also fit the other encoding.
    pub encoding: NameEncoding,
}

/// One import-table entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportEntry {
    pub class_package_index: i32,
    pub class_name_index: i32,
    pub outer_ref: i32,
    pub object_name_index: i32,
}

/// One export-table entry. The serial payload is referenced by
/// (`serial_offset`, `serial_size`) into the source bytes, never copied into
/// the entry itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportEntry {
    pub class_ref: i32,
    pub super_ref: i32,
    pub outer_ref: i32,
    pub object_name_index: i32,
    pub object_flags: u32,
    pub serial_size: i32,
    /// Present exactly when `serial_size != 0`.
    pub serial_offset: Option<i32>,
}

fn bad(detail: &'static str) -> PackageError {
    layout(detail)
}

/// Read a >=64 name FString with the reference reader's strict checks:
/// 1..=MAX_NAME_UNITS units (count includes the NUL), singly NUL-terminated,
/// non-empty text.
fn read_name_string(cur: &mut ByteCursor<'_>) -> PackageResult<(String, NameEncoding)> {
    let count = read_compact_index(cur)?;
    let units = count.unsigned_abs() as usize;
    if units == 0 {
        return Err(bad("name FString has zero serialized characters"));
    }
    if units > MAX_NAME_UNITS {
        return Err(layout(format!(
            "name FString has {units} units; maximum is {MAX_NAME_UNITS}"
        )));
    }
    let entry = if count > 0 {
        let raw = cur.take(units)?;
        if raw[raw.len() - 1] != 0 || raw[..raw.len() - 1].contains(&0) {
            return Err(bad("ANSI name FString is not singly NUL-terminated"));
        }
        (latin1_str(&raw[..raw.len() - 1]), NameEncoding::Ansi)
    } else {
        let raw = cur.take(units * 2)?;
        let units16: Vec<u16> = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        if units16.last() != Some(&0) || units16[..units16.len() - 1].contains(&0) {
            return Err(bad("UTF-16 name FString is not singly NUL-terminated"));
        }
        let text: String = String::from_utf16(&units16[..units16.len() - 1])
            .map_err(|_| bad("invalid UTF-16LE name FString"))?;
        (text, NameEncoding::Utf16)
    };
    if entry.0.is_empty() {
        return Err(bad("empty package name entry"));
    }
    Ok(entry)
}

/// Read a pre-64 name entry: NUL-terminated ANSI bytes without a length
/// prefix.
fn read_pre_64_name(cur: &mut ByteCursor<'_>) -> PackageResult<String> {
    let start = cur.position();
    let data = cur.data();
    let end = data[start..]
        .iter()
        .position(|&b| b == 0)
        .map(|rel| start + rel)
        .ok_or_else(|| bad("pre-64 ANSI name entry has no NUL terminator"))?;
    if end - start + 1 > MAX_NAME_UNITS {
        return Err(layout(format!(
            "pre-64 ANSI name entry exceeds {MAX_NAME_UNITS} units"
        )));
    }
    let raw = cur.take(end - start + 1)?;
    let text = latin1_str(&raw[..raw.len() - 1]);
    if text.is_empty() {
        return Err(bad("empty package name entry"));
    }
    Ok(text)
}

/// Parse `summary.name_count` consecutive name entries starting at
/// `summary.name_offset`; returns the entries and the end offset of the name
/// region.
pub fn parse_names(
    data: &[u8],
    summary: &PackageSummary,
) -> PackageResult<(Vec<NameEntry>, usize)> {
    let mut cur = ByteCursor::at(data, summary.name_offset as usize);
    let pre_64 = summary.pre_64_names();
    let mut names = Vec::new();
    for _ in 0..summary.name_count {
        let entry = if pre_64 {
            NameEntry {
                text: read_pre_64_name(&mut cur)?,
                flags: cur.u32()?,
                encoding: NameEncoding::Ansi,
            }
        } else {
            let (text, encoding) = read_name_string(&mut cur)?;
            NameEntry {
                text,
                flags: cur.u32()?,
                encoding,
            }
        };
        names.push(entry);
    }
    Ok((names, cur.position()))
}

/// Parse `summary.import_count` import entries starting at
/// `summary.import_offset`; validates every name index against `names`.
pub fn parse_imports(
    data: &[u8],
    summary: &PackageSummary,
    names: &[NameEntry],
) -> PackageResult<(Vec<ImportEntry>, usize)> {
    let mut cur = ByteCursor::at(data, summary.import_offset as usize);
    let mut imports = Vec::new();
    for _ in 0..summary.import_count {
        let class_package_index = read_compact_index(&mut cur)?;
        let class_name_index = read_compact_index(&mut cur)?;
        let outer_ref = cur.i32()?;
        let object_name_index = read_compact_index(&mut cur)?;
        for index in [class_package_index, class_name_index, object_name_index] {
            if index < 0 || index as usize >= names.len() {
                return Err(PackageError::BadNameIndex {
                    index,
                    count: names.len(),
                });
            }
        }
        imports.push(ImportEntry {
            class_package_index,
            class_name_index,
            outer_ref,
            object_name_index,
        });
    }
    Ok((imports, cur.position()))
}

/// Parse `summary.export_count` export entries starting at
/// `summary.export_offset`.
pub fn parse_exports(
    data: &[u8],
    summary: &PackageSummary,
    names: &[NameEntry],
) -> PackageResult<Vec<ExportEntry>> {
    let mut cur = ByteCursor::at(data, summary.export_offset as usize);
    let mut exports = Vec::new();
    for _ in 0..summary.export_count {
        let class_ref = read_compact_index(&mut cur)?;
        let super_ref = read_compact_index(&mut cur)?;
        let outer_ref = cur.i32()?;
        let object_name_index = read_compact_index(&mut cur)?;
        let object_flags = cur.u32()?;
        let serial_size = read_compact_index(&mut cur)?;
        if serial_size < 0 {
            return Err(layout(format!("negative export serial size {serial_size}")));
        }
        let serial_offset = if serial_size != 0 {
            let offset = read_compact_index(&mut cur)?;
            if offset < 0 {
                return Err(layout(format!("negative export serial offset {offset}")));
            }
            Some(offset)
        } else {
            None
        };
        if object_name_index < 0 || object_name_index as usize >= names.len() {
            return Err(PackageError::BadNameIndex {
                index: object_name_index,
                count: names.len(),
            });
        }
        exports.push(ExportEntry {
            class_ref,
            super_ref,
            outer_ref,
            object_name_index,
            object_flags,
            serial_size,
            serial_offset,
        });
    }
    Ok(exports)
}

/// Serialize one name entry using its preserved original encoding (or the
/// pre-64 unprefixed form).
pub fn write_name_entry(out: &mut Vec<u8>, entry: &NameEntry, pre_64: bool) {
    if pre_64 {
        out.extend_from_slice(
            entry
                .text
                .chars()
                .map(|c| c as u32 as u8)
                .collect::<Vec<u8>>()
                .as_slice(),
        );
        out.push(0);
    } else {
        match entry.encoding {
            NameEncoding::Ansi => {
                let bytes: Vec<u8> = entry.text.chars().map(|c| c as u32 as u8).collect();
                write_compact_index(out, bytes.len() as i32 + 1);
                out.extend_from_slice(&bytes);
                out.push(0);
            }
            NameEncoding::Utf16 => {
                let mut units: Vec<u16> = entry.text.encode_utf16().collect();
                units.push(0);
                write_compact_index(out, -(units.len() as i32));
                for unit in units {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
            }
        }
    }
    out.extend_from_slice(&entry.flags.to_le_bytes());
}

/// Serialize one import entry.
pub fn write_import_entry(out: &mut Vec<u8>, entry: &ImportEntry) {
    write_compact_index(out, entry.class_package_index);
    write_compact_index(out, entry.class_name_index);
    out.extend_from_slice(&entry.outer_ref.to_le_bytes());
    write_compact_index(out, entry.object_name_index);
}

/// Serialize one export entry.
pub fn write_export_entry(out: &mut Vec<u8>, entry: &ExportEntry) {
    write_compact_index(out, entry.class_ref);
    write_compact_index(out, entry.super_ref);
    out.extend_from_slice(&entry.outer_ref.to_le_bytes());
    write_compact_index(out, entry.object_name_index);
    out.extend_from_slice(&entry.object_flags.to_le_bytes());
    write_compact_index(out, entry.serial_size);
    if entry.serial_size != 0 {
        write_compact_index(out, entry.serial_offset.unwrap_or_default());
    }
}
