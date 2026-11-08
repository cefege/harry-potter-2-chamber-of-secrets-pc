//! Owned package79 archive bytes and parsed linker tables for Rust consumers.
//!
//! The archive keeps the original package bytes and exposes bounded views rather
//! than constructing engine objects or translating packages into another format.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core_access::{RuntimeError, RuntimeResult};


/// Summary fields and table locations serialized in a package header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSummary {
    pub tag: u32,
    pub version: u16,
    pub licensee_version: u16,
    pub package_flags: u32,
    pub name_count: u32,
    pub name_offset: u32,
    pub export_count: u32,
    pub export_offset: u32,
    pub import_count: u32,
    pub import_offset: u32,
    pub guid: [u32; 4],
    pub generations: Vec<GenerationInfo>,
}

/// Per-generation export and name counts from version-68 package summaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationInfo {
    pub export_count: i32,
    pub name_count: i32,
}

/// Package-local name entry, retaining its source byte interval and flags.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageName {
    pub text: Vec<u32>,
    pub flags: u32,
    pub offset: u32,
    pub end_offset: u32,
}

/// Import record indices as serialized in the package79 table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageImport {
    pub class_package: i32,
    pub class_name: i32,
    pub package_index: i32,
    pub object_name: i32,
    pub offset: u32,
    pub end_offset: u32,
}

/// Export record, including its original bounded serial-data range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageExport {
    pub class_index: i32,
    pub super_index: i32,
    pub package_index: i32,
    pub object_name: i32,
    pub object_flags: u32,
    pub serial_size: i32,
    pub serial_offset: i32,
    pub offset: u32,
    pub end_offset: u32,
}

/// A package-local object reference resolved to its import/export table row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageObjectRef {
    /// A null `FPackageIndex`.
    Null,
    /// A one-based positive export index converted to a zero-based row.
    Export(usize),
    /// A one-based negative import index converted to a zero-based row.
    Import(usize),
}

#[derive(Clone, Debug)]
enum ExportBytes<'a> {
    Borrowed(&'a [u8]),
    Shared { bytes: std::rc::Rc<Vec<u8>>, range: std::ops::Range<usize> },
}

impl ExportBytes<'_> {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Shared { bytes, range } => &bytes[range.clone()],
        }
    }
}

/// A read-only archive view constrained to one export's serialized payload.
#[derive(Clone, Debug)]
pub struct ExportArchive<'a> {
    storage: ExportBytes<'a>,
    position: usize,
}

impl<'a> ExportArchive<'a> {
    /// Creates a cursor over an in-memory byte slice.
    pub fn from_slice(bytes: &'a [u8]) -> Self {
        Self { storage: ExportBytes::Borrowed(bytes), position: 0 }
    }

    /// Creates a bounded cursor over shared package bytes without copying payload.
    pub fn from_shared_range(bytes: std::rc::Rc<Vec<u8>>, range: std::ops::Range<usize>) -> RuntimeResult<ExportArchive<'static>> {
        if range.start > range.end || range.end > bytes.len() {
            return Err(format_error("export serial range outside package"));
        }
        Ok(ExportArchive { storage: ExportBytes::Shared { bytes, range }, position: 0 })
    }

    fn bytes(&self) -> &[u8] { self.storage.as_slice() }
    pub(crate) fn shared_source_range(
        &self,
    ) -> Option<(std::rc::Rc<Vec<u8>>, std::ops::Range<usize>)> {
        match &self.storage {
            ExportBytes::Borrowed(_) => None,
            ExportBytes::Shared { bytes, range } => Some((bytes.clone(), range.clone())),
        }
    }
    pub(crate) fn shared_source_origin(&self) -> Option<usize> {
        match &self.storage {
            ExportBytes::Borrowed(_) => None,
            ExportBytes::Shared { range, .. } => Some(range.start),
        }
    }
}

impl<'a> ExportArchive<'a> {
    /// Returns the payload length in bytes.
    pub fn len(&self) -> usize { self.bytes().len() }
    /// Returns whether the bounded payload is empty.
    pub fn is_empty(&self) -> bool { self.bytes().is_empty() }
    /// Returns the current position relative to the payload.
    pub fn tell(&self) -> usize { self.position }
    /// Seeks within this export; positions beyond its end are rejected.
    pub fn seek(&mut self, position: usize) -> RuntimeResult<()> {
        if position > self.bytes().len() { return Err(format_error("export seek outside payload")); }
        self.position = position;
        Ok(())
    }
    /// Reads bytes without crossing this export's serial boundary.
    pub fn read(&mut self, count: usize) -> RuntimeResult<&[u8]> {
        let end = self.position.checked_add(count).ok_or_else(|| format_error("export read overflow"))?;
        let value = self.storage.as_slice().get(self.position..end).ok_or_else(|| format_error("truncated export payload"))?;
        self.position = end;
        Ok(value)
    }
    /// Reads directly into caller-owned storage without exposing a borrow to the cursor.
    pub fn read_into(&mut self, output: &mut [u8]) -> RuntimeResult<()> {
        let bytes = self.read(output.len())?;
        output.copy_from_slice(bytes);
        Ok(())
    }

    /// Reads one byte from the export payload.
    pub fn read_u8(&mut self) -> RuntimeResult<u8> { Ok(self.read(1)?[0]) }
    /// Reads a little-endian unsigned 16-bit archive word.
    pub fn read_u16(&mut self) -> RuntimeResult<u16> {
        let bytes = self.read(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    /// Reads a little-endian IEEE-754 single-precision value.
    pub fn read_f32(&mut self) -> RuntimeResult<f32> { Ok(f32::from_bits(self.read_u32()?)) }
    /// Reads a little-endian signed archive integer.
    pub fn read_i32(&mut self) -> RuntimeResult<i32> {
        Ok(i32::from_le_bytes(self.read(4)?.try_into().map_err(|_| format_error("truncated export integer"))?))
    }
    /// Reads a little-endian unsigned archive DWORD.
    pub fn read_u32(&mut self) -> RuntimeResult<u32> {
        Ok(u32::from_le_bytes(self.read(4)?.try_into().map_err(|_| format_error("truncated export DWORD"))?))
    }
    /// Reads Unreal's signed one-to-five-byte compact index encoding.
    pub fn read_compact_index(&mut self) -> RuntimeResult<i32> {
        crate::compact_index::decode(|| Ok(self.read(1)?[0]))
    }
    /// Reads an ANSI-or-UTF-16 FString and returns Unicode scalar values.
    pub fn read_fstring(&mut self) -> RuntimeResult<Vec<u32>> {
        let length = self.read_compact_index()?;
        let count = length.checked_abs().ok_or_else(|| format_error("invalid export string length"))? as usize;
        let width = if length < 0 { 2 } else { 1 };
        if count.checked_mul(width).filter(|size| *size <= self.bytes().len().saturating_sub(self.position)).is_none() {
            return Err(format_error("export string length outside payload"));
        }
        crate::fstring_archive::decode_utf32(length, |unit| match unit {
            crate::fstring_archive::FStringWireUnit::Ansi => Ok(u16::from(self.read(1)?[0])),
            crate::fstring_archive::FStringWireUnit::Utf16 => {
                let bytes = self.read(2)?;
                Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
            }
        })
    }
}

/// Owned package data, summary, and fully parsed linker tables.
#[derive(Debug)]
pub struct OwnedPackage {
    path: Option<PathBuf>,
    bytes: Vec<u8>,
    summary: PackageSummary,
    names: Vec<PackageName>,
    imports: Vec<PackageImport>,
    exports: Vec<PackageExport>,
}
/// Owned package components for transferring parsed metadata to the canonical linker.
#[derive(Debug)]
pub struct PackageParts {
    pub path: Option<PathBuf>,
    pub bytes: std::rc::Rc<Vec<u8>>,
    pub summary: PackageSummary,
    pub names: Vec<PackageName>,
    pub imports: Vec<PackageImport>,
    pub exports: Vec<PackageExport>,
}
impl PackageParts {
    /// Creates a bounded cursor over an export without copying package payload bytes.
    pub fn export_archive(&self, index: usize) -> RuntimeResult<ExportArchive<'static>> {
        let export = self.exports.get(index).ok_or_else(|| format_error("export index out of range"))?;
        shared_export_archive(self.bytes.clone(), export)
    }
}

impl OwnedPackage {
    /// Moves the original bytes and parsed tables out without cloning them.
    pub fn into_parts(self) -> PackageParts {
        PackageParts {
            path: self.path,
            bytes: std::rc::Rc::new(self.bytes),
            summary: self.summary,
            names: self.names,
            imports: self.imports,
            exports: self.exports,
        }
    }
}


impl OwnedPackage {
    /// Opens and parses a package from disk, preserving its original bytes.
    pub fn open(path: impl AsRef<Path>) -> RuntimeResult<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|error| {
            let context = format!("{}: {error}", path.display());
            RuntimeError::new("failed to read package").with_context(&context)
        })?;
        Self::from_bytes_at(bytes, Some(path.to_path_buf()))
    }

    /// Parses owned bytes as a package79 file.
    pub fn from_bytes(bytes: Vec<u8>) -> RuntimeResult<Self> { Self::from_bytes_at(bytes, None) }

    fn from_bytes_at(bytes: Vec<u8>, path: Option<PathBuf>) -> RuntimeResult<Self> {
        let parsed = (|| {
            let summary = parse_summary(&bytes)?;
            let names = parse_names(&bytes, &summary).map_err(|error| error.with_context("LoadNameMap"))?;
            let imports = parse_imports(&bytes, &summary).map_err(|error| error.with_context("LoadImportMap"))?;
            let exports = parse_exports(&bytes, &summary).map_err(|error| error.with_context("LoadExportMap"))?;
            Ok((summary, names, imports, exports))
        })();
        let (summary, names, imports, exports) = parsed.map_err(|error: RuntimeError| {
            match path.as_deref() {
                Some(path) => error.with_source_path(path),
                None => error,
            }
        })?;
        Ok(Self { path, bytes, summary, names, imports, exports })
    }

    /// Original source path, if this package was opened from a file.
    pub fn path(&self) -> Option<&Path> { self.path.as_deref() }
    /// Original package bytes.
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    /// Writes the preserved bytes to a destination without changing their format.
    pub fn write_to(&self, path: impl AsRef<Path>) -> RuntimeResult<()> {
        let path = path.as_ref();
        fs::write(path, &self.bytes).map_err(|error| {
            let context = format!("{}: {error}", path.display());
            RuntimeError::new("failed to write package").with_context(&context)
        })
    }
    /// Parsed package summary.
    pub fn summary(&self) -> &PackageSummary { &self.summary }
    /// Parsed package-local name table.
    pub fn names(&self) -> &[PackageName] { &self.names }
    /// Parsed import table.
    pub fn imports(&self) -> &[PackageImport] { &self.imports }
    /// Parsed export table.

    pub fn exports(&self) -> &[PackageExport] { &self.exports }
    /// Returns a checked cursor over one export's serialized payload.
    pub fn export_archive(&self, index: usize) -> RuntimeResult<ExportArchive<'_>> {
        let export = self.exports.get(index).ok_or_else(|| format_error("export index out of range"))?;
        let range = export_range(export, self.bytes.len())?;
        Ok(ExportArchive::from_slice(&self.bytes[range]))
    }

    /// Resolves a package-local name-table index to its UTF-32 spelling.
    pub fn name_text(&self, index: i32) -> Option<&[u32]> {
        self.names.get(usize::try_from(index).ok()?).map(|name| name.text.as_slice())
    }
    /// Resolves an `FPackageIndex` without constructing or registering an object.
    pub fn resolve_object_index(&self, index: i32) -> RuntimeResult<PackageObjectRef> {
        if index == 0 { return Ok(PackageObjectRef::Null); }
        if index > 0 {
            let row = (index - 1) as usize;
            if row < self.exports.len() { Ok(PackageObjectRef::Export(row)) }
            else { Err(format_error("export reference out of range")) }
        } else {
            let row = index.unsigned_abs().wrapping_sub(1) as usize;
            if row < self.imports.len() { Ok(PackageObjectRef::Import(row)) }
            else { Err(format_error("import reference out of range")) }
        }
    }

    /// Returns outer-to-inner name components for a serialized package index.
    ///
    /// Zero denotes the package root, positive indices select exports, and
    /// negative indices select imports, matching `FPackageIndex`.
    pub fn object_path(&self, package_index: i32, object_name: i32) -> RuntimeResult<Vec<&[u32]>> {
        let mut chain = Vec::new();
        let mut current = package_index;
        let mut depth = 0usize;
        while current != 0 {
            depth = depth.checked_add(1).ok_or_else(|| format_error("package outer depth overflow"))?;
            if depth > self.exports.len().saturating_add(self.imports.len()) {
                return Err(format_error("cyclic package outer chain"));
            }
            if current > 0 {
                let record = self.exports.get((current - 1) as usize).ok_or_else(|| format_error("export reference out of range"))?;
                chain.push(self.name_text(record.object_name).ok_or_else(|| format_error("export name index out of range"))?);
                current = record.package_index;
            } else {
                let record = self.imports.get(current.unsigned_abs().wrapping_sub(1) as usize).ok_or_else(|| format_error("import reference out of range"))?;
                chain.push(self.name_text(record.object_name).ok_or_else(|| format_error("import name index out of range"))?);
                current = record.package_index;
            }
        }
        chain.reverse();
        chain.push(self.name_text(object_name).ok_or_else(|| format_error("object name index out of range"))?);
        Ok(chain)
    }
}
/// Creates an export cursor backed by shared original package bytes.
pub fn shared_export_archive(bytes: std::rc::Rc<Vec<u8>>, export: &PackageExport) -> RuntimeResult<ExportArchive<'static>> {
    let range = export_range(export, bytes.len())?;
    ExportArchive::from_shared_range(bytes, range)
}
/// Writable package archive cursor used for package79 primitive and table output.
#[derive(Clone, Debug, Default)]
pub struct PackageArchiveWriter {
    bytes: Vec<u8>,
    position: usize,
}

impl PackageArchiveWriter {
    /// Creates an empty writer.
    pub fn new() -> Self { Self::default() }
    /// Creates a writer over existing package bytes, positioned at their end.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let position = bytes.len();
        Self { bytes, position }
    }
    /// Returns the writer cursor.
    pub fn tell(&self) -> usize { self.position }
    /// Returns the emitted bytes.
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    /// Writes the assembled package bytes to a destination.
    pub fn write_to(&self, path: impl AsRef<Path>) -> RuntimeResult<()> {
        let path = path.as_ref();
        fs::write(path, &self.bytes).map_err(|error| {
            let context = format!("{}: {error}", path.display());
            RuntimeError::new("failed to write package").with_context(&context)
        })
    }
    /// Consumes the writer and returns its bytes.
    pub fn into_bytes(self) -> Vec<u8> { self.bytes }
    /// Seeks to an existing position or the end; sparse extension is rejected.
    pub fn seek(&mut self, position: usize) -> RuntimeResult<()> {
        if position > self.bytes.len() { return Err(format_error("package seek outside written bytes")); }
        self.position = position;
        Ok(())
    }
    /// Writes raw bytes at the current cursor, replacing or extending as needed.
    pub fn write(&mut self, bytes: &[u8]) -> RuntimeResult<()> {
        let end = self.position.checked_add(bytes.len()).ok_or_else(|| format_error("package write overflow"))?;
        if end > self.bytes.len() { self.bytes.resize(end, 0); }
        self.bytes[self.position..end].copy_from_slice(bytes);
        self.position = end;
        Ok(())
    }
    /// Writes a little-endian signed archive integer.
    pub fn write_i32(&mut self, value: i32) -> RuntimeResult<()> { self.write(&value.to_le_bytes()) }
    /// Writes a little-endian unsigned archive DWORD.
    pub fn write_u32(&mut self, value: u32) -> RuntimeResult<()> { self.write(&value.to_le_bytes()) }
    /// Writes one byte.
    pub fn write_u8(&mut self, value: u8) -> RuntimeResult<()> { self.write(&[value]) }
    /// Writes a little-endian unsigned 16-bit archive word.
    pub fn write_u16(&mut self, value: u16) -> RuntimeResult<()> { self.write(&value.to_le_bytes()) }
    /// Writes a little-endian IEEE-754 single-precision value.
    pub fn write_f32(&mut self, value: f32) -> RuntimeResult<()> { self.write_u32(value.to_bits()) }
    /// Writes Unreal's signed one-to-five-byte compact index encoding.
    pub fn write_compact_index(&mut self, value: i32) -> RuntimeResult<()> {
        crate::compact_index::encode(value, |byte| {
            self.write(&[byte])?;
            Ok(byte)
        })?;
        Ok(())
    }
    /// Writes a package79 FString in ANSI form when all scalars fit one byte,
    /// otherwise as UTF-16 code units, including the archive terminator.
    pub fn write_fstring(&mut self, text: &[u32]) -> RuntimeResult<()> {
        let length = crate::fstring_archive::utf32_wire_length(text)
            .ok_or_else(|| format_error("package string length overflow"))?;
        self.write_compact_index(length)?;
        crate::fstring_archive::encode_utf32(text, length > 0, |unit, value| match unit {
            crate::fstring_archive::FStringWireUnit::Ansi => self.write(&[value as u8]),
            crate::fstring_archive::FStringWireUnit::Utf16 => self.write(&value.to_le_bytes()),
        })
    }
    /// Writes a package name-table entry using the version-specific name encoding.
    pub fn write_name(&mut self, name: &PackageName, version: u16) -> RuntimeResult<()> {
        if version < 64 {
            if name.text.len() + 1 > 128 || name.text.iter().any(|scalar| *scalar == 0 || *scalar > 0xff) {
                return Err(format_error("name cannot be represented in pre-64 ANSI format"));
            }
            for scalar in &name.text { self.write(&[*scalar as u8])?; }
            self.write(&[0])?;
            self.write_u32(name.flags)
        } else {
            self.write_fstring(&name.text)?;
            self.write_u32(name.flags)
        }
    }
    /// Writes an import-table record in the established package79 field order.
    pub fn write_import(&mut self, record: &PackageImport) -> RuntimeResult<()> {
        crate::linker_record_archive::write_import_fields(self, &crate::linker_record_archive::ImportFields {
            class_package: record.class_package,
            class_name: record.class_name,
            package_index: record.package_index,
            object_name: record.object_name,
        })
    }
    /// Writes an export-table record, omitting SerialOffset for zero-sized exports.
    pub fn write_export(&mut self, record: &PackageExport) -> RuntimeResult<()> {
        crate::linker_record_archive::write_export_fields(self, &crate::linker_record_archive::ExportFields {
            class_index: record.class_index,
            super_index: record.super_index,
            package_index: record.package_index,
            object_name: record.object_name,
            object_flags: record.object_flags,
            serial_size: record.serial_size,
            serial_offset: record.serial_offset,
        })
    }
}

/// Serializes a package summary using the legacy version-dependent wire fields.
pub fn write_package_summary(writer: &mut PackageArchiveWriter, summary: &PackageSummary) -> RuntimeResult<()> {
    crate::package_summary_archive::write_owned_summary(writer, summary)
}

impl crate::package_summary_archive::PackageSummaryOutput for PackageArchiveWriter {
    type Error = RuntimeError;
    fn error(&self, message: &str) -> RuntimeError { RuntimeError::new(message) }
    fn write_i32(&mut self, value: i32) -> RuntimeResult<()> { PackageArchiveWriter::write_i32(self, value) }
    fn write_u32(&mut self, value: u32) -> RuntimeResult<()> { PackageArchiveWriter::write_u32(self, value) }
}

fn format_error(message: &str) -> RuntimeError { RuntimeError::new(message) }
fn export_range(export: &PackageExport, length: usize) -> RuntimeResult<std::ops::Range<usize>> {
    if export.serial_size < 0 || export.serial_offset < 0 {
        return Err(format_error("invalid export serial range"));
    }
    let start = export.serial_offset as usize;
    let end = start.checked_add(export.serial_size as usize)
        .ok_or_else(|| format_error("export serial range overflow"))?;
    if end > length { return Err(format_error("export serial range outside package")); }
    Ok(start..end)
}

struct Reader<'a> { bytes: &'a [u8], position: usize }
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, position: 0 } }
    fn seek(&mut self, position: usize) -> RuntimeResult<()> {
        if position > self.bytes.len() {
            return Err(format_error("Invalid file-reader seek").with_context("ULinkerLoad::Seek"));
        }
        self.position = position;
        Ok(())
    }
    fn take(&mut self, count: usize) -> RuntimeResult<&'a [u8]> {
        let end = self.position.checked_add(count).ok_or_else(|| format_error("package offset overflow"))?;
        let value = self.bytes.get(self.position..end).ok_or_else(|| format_error("truncated package"))?;
        self.position = end;
        Ok(value)
    }
    fn i32(&mut self) -> RuntimeResult<i32> { Ok(i32::from_le_bytes(self.take(4)?.try_into().map_err(|_| format_error("truncated package integer"))?)) }
    fn u32(&mut self) -> RuntimeResult<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| format_error("truncated package integer"))?)) }
    fn compact(&mut self) -> RuntimeResult<i32> {
        crate::compact_index::decode(|| Ok(self.take(1)?[0]))
    }
}

impl crate::package_summary_archive::PackageSummaryInput for Reader<'_> {
    type Error = RuntimeError;
    fn error(&self, message: &str) -> RuntimeError { RuntimeError::new(message) }
    fn tell(&self) -> usize { self.position }
    fn seek(&mut self, position: usize) -> RuntimeResult<()> { Reader::seek(self, position) }
    fn read_i32(&mut self) -> RuntimeResult<i32> { self.i32() }
    fn read_u32(&mut self) -> RuntimeResult<u32> { self.u32() }
}

fn parse_summary(bytes: &[u8]) -> RuntimeResult<PackageSummary> {
    crate::package_summary_archive::read_owned_summary(&mut Reader::new(bytes))
}

impl crate::linker_record_archive::LinkerRecordInput for Reader<'_> {
    type Error = RuntimeError;
    fn compact_index(&mut self) -> RuntimeResult<i32> { self.compact() }
    fn int(&mut self) -> RuntimeResult<i32> { self.i32() }
    fn dword(&mut self) -> RuntimeResult<u32> { self.u32() }
}

impl crate::linker_record_archive::LinkerRecordOutput for PackageArchiveWriter {
    type Error = RuntimeError;
    fn compact_index(&mut self, value: i32) -> RuntimeResult<()> { self.write_compact_index(value) }
    fn int(&mut self, value: i32) -> RuntimeResult<()> { self.write_i32(value) }
    fn dword(&mut self, value: u32) -> RuntimeResult<()> { self.write_u32(value) }
}
fn ensure_table_fits(bytes: &[u8], offset: u32, count: u32, minimum_record_size: usize) -> RuntimeResult<()> {
    let start = offset as usize;
    let available = bytes.len().checked_sub(start).ok_or_else(|| format_error("package table offset outside file"))?;
    if count as usize > available / minimum_record_size {
        return Err(format_error("package table count exceeds available bytes"));
    }
    Ok(())
}

fn parse_names(bytes: &[u8], summary: &PackageSummary) -> RuntimeResult<Vec<PackageName>> {
    let mut r = Reader::new(bytes); r.seek(summary.name_offset as usize)?;
    ensure_table_fits(bytes, summary.name_offset, summary.name_count, 5)?;
    let mut names = Vec::with_capacity(summary.name_count as usize);
    for _ in 0..summary.name_count {
        let offset = r.position as u32;
        let text = if summary.version < 64 {
            let remaining = bytes.get(r.position..).ok_or_else(|| format_error("package name offset outside file"))?;
            let length = remaining.iter().position(|byte| *byte == 0)
                .ok_or_else(|| format_error("pre-64 ANSI name has no NUL terminator"))?;
            if length + 1 > 128 { return Err(format_error("pre-64 ANSI name exceeds 128 bytes")); }
            r.take(length + 1)?[..length].iter().map(|byte| u32::from(*byte)).collect()
        } else {
            let len = r.compact()?;
            let units = len.checked_abs().ok_or_else(|| format_error("invalid package string length"))? as usize;
            let width = if len < 0 { 2 } else { 1 };
            if units.checked_mul(width).filter(|size| *size <= bytes.len().saturating_sub(r.position)).is_none() {
                return Err(format_error("package string length outside file"));
            }
            crate::fstring_archive::decode_utf32(len, |unit| match unit {
                crate::fstring_archive::FStringWireUnit::Ansi => Ok(u16::from(r.take(1)?[0])),
                crate::fstring_archive::FStringWireUnit::Utf16 => {
                    let bytes = r.take(2)?;
                    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
                }
            })?
        };
        let flags = r.u32()?;
        names.push(PackageName { text, flags, offset, end_offset: r.position as u32 });
    }
    Ok(names)
}

fn parse_imports(bytes: &[u8], summary: &PackageSummary) -> RuntimeResult<Vec<PackageImport>> {
    let mut r = Reader::new(bytes); r.seek(summary.import_offset as usize)?;
    ensure_table_fits(bytes, summary.import_offset, summary.import_count, 7)?;
    let mut records = Vec::with_capacity(summary.import_count as usize);
    for _ in 0..summary.import_count {
        let offset = r.position as u32;
        let fields = crate::linker_record_archive::read_import_fields(&mut r)?;
        crate::linker_name_archive::validate_local_index(fields.class_package, summary.name_count as i32, |_, _| format_error("import class package name index out of range"))?;
        crate::linker_name_archive::validate_local_index(fields.class_name, summary.name_count as i32, |_, _| format_error("import class name index out of range"))?;
        crate::linker_name_archive::validate_local_index(fields.object_name, summary.name_count as i32, |_, _| format_error("import object name index out of range"))?;
        records.push(PackageImport {
            class_package: fields.class_package,
            class_name: fields.class_name,
            package_index: fields.package_index,
            object_name: fields.object_name,
            offset,
            end_offset: r.position as u32,
        });
    }
    Ok(records)
}

fn parse_exports(bytes: &[u8], summary: &PackageSummary) -> RuntimeResult<Vec<PackageExport>> {
    let mut r = Reader::new(bytes); r.seek(summary.export_offset as usize)?;
    ensure_table_fits(bytes, summary.export_offset, summary.export_count, 12)?;
    let mut records = Vec::with_capacity(summary.export_count as usize);
    for _ in 0..summary.export_count {
        let offset = r.position as u32;
        let fields = crate::linker_record_archive::read_export_fields(&mut r)?;
        crate::linker_name_archive::validate_local_index(fields.object_name, summary.name_count as i32, |_, _| format_error("export object name index out of range"))?;
        records.push(PackageExport {
            class_index: fields.class_index,
            super_index: fields.super_index,
            package_index: fields.package_index,
            object_name: fields.object_name,
            object_flags: fields.object_flags,
            serial_size: fields.serial_size,
            serial_offset: fields.serial_offset,
            offset,
            end_offset: r.position as u32,
        });
    }
    Ok(records)
}
