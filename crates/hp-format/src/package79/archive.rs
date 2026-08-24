//! Whole-archive reader and byte-exact writer.
//!
//! `read_package` mirrors `Build/package79_reference.py::PackageReader.parse`
//! including region validation; `write_package` reproduces the exact input
//! bytes for anything that parsed.

use super::error::{PackageError, PackageResult, layout};
use super::summary::{check_offset, parse_summary};
use super::tables::{self, ExportEntry, ImportEntry, NameEntry};

/// A fully parsed package. Export data payloads stay referenced by
/// (`serial_offset`, `serial_size`) into the contiguous payload region held
/// here — individual payloads are never copied into entries.
#[derive(Debug, Clone)]
pub struct PackageArchive {
    pub summary: super::summary::PackageSummary,
    pub names: Vec<NameEntry>,
    pub imports: Vec<ImportEntry>,
    pub exports: Vec<ExportEntry>,
    /// End offset of the name region == start offset of the payload region.
    names_end: usize,
    /// Verbatim bytes of `[names_end, import_offset)`.
    payload_region: Vec<u8>,
}

impl PackageArchive {
    /// The contiguous export-payload region (offsets are absolute file
    /// offsets starting at [`Self::names_end`]).
    pub fn payload_bytes(&self) -> &[u8] {
        &self.payload_region
    }

    /// End offset of the name region / start of the payload region.
    pub fn names_end(&self) -> usize {
        self.names_end
    }

    /// Borrow export `index`'s serial payload from the source bytes.
    pub fn export_payload(&self, index: usize) -> Option<&[u8]> {
        let entry = self.exports.get(index)?;
        let offset = entry.serial_offset? as usize;
        if entry.serial_size <= 0 {
            return None;
        }
        let start = offset.checked_sub(self.names_end)?;
        self.payload_region
            .get(start..start + entry.serial_size as usize)
    }
}

/// Read a UE1 package (FileVersion 60..=79, LicenseeVersion 0) from `bytes`.
pub fn read_package(bytes: &[u8]) -> Result<PackageArchive, PackageError> {
    let summary = parse_summary(bytes)?;
    let (names, names_end) = tables::parse_names(bytes, &summary)?;
    let (imports, imports_end) = tables::parse_imports(bytes, &summary, &names)?;
    let exports = tables::parse_exports(bytes, &summary, &names)?;
    let exports_end = bytes.len();

    for entry in &imports {
        check_object_ref(
            entry.outer_ref,
            "import outer",
            exports.len(),
            imports.len(),
        )?;
    }
    for entry in &exports {
        check_object_ref(
            entry.class_ref,
            "export class",
            exports.len(),
            imports.len(),
        )?;
        check_object_ref(
            entry.super_ref,
            "export super",
            exports.len(),
            imports.len(),
        )?;
        check_object_ref(
            entry.outer_ref,
            "export outer",
            exports.len(),
            imports.len(),
        )?;
    }

    validate_regions(
        &summary,
        names_end,
        &exports,
        imports_end,
        exports_end,
        bytes.len(),
    )?;

    detect_outer_cycles(&imports, &exports)?;

    let payload_start = names_end;
    let payload_end = summary.import_offset as usize;
    Ok(PackageArchive {
        summary,
        names,
        imports,
        exports,
        names_end,
        payload_region: bytes[payload_start..payload_end].to_vec(),
    })
}

fn check_object_ref(
    value: i32,
    label: &str,
    export_count: usize,
    import_count: usize,
) -> PackageResult<()> {
    if value > 0 && value as usize > export_count {
        return Err(layout(format!(
            "{label} export reference {value} is out of range"
        )));
    }
    if value < 0 && (-value) as usize > import_count {
        return Err(layout(format!(
            "{label} import reference {value} is out of range"
        )));
    }
    Ok(())
}

fn validate_regions(
    summary: &super::summary::PackageSummary,
    names_end: usize,
    exports: &[ExportEntry],
    imports_end: usize,
    exports_end: usize,
    file_len: usize,
) -> PackageResult<()> {
    let import_offset = summary.import_offset as usize;
    let export_offset = summary.export_offset as usize;

    if !(names_end <= import_offset && import_offset <= export_offset) {
        return Err(layout(
            "name/payload/import/export regions are out of order",
        ));
    }
    if imports_end != export_offset {
        return Err(layout(format!(
            "import table ends at {imports_end}, expected export offset {export_offset}"
        )));
    }
    if exports_end != file_len {
        return Err(layout(format!(
            "export table ends at {exports_end}, expected file size {file_len}"
        )));
    }

    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    for (index, entry) in exports.iter().enumerate() {
        if entry.serial_size == 0 {
            if entry.serial_offset.is_some() {
                return Err(layout("zero-size export unexpectedly has an offset"));
            }
            continue;
        }
        let Some(offset) = entry.serial_offset else {
            return Err(layout(format!("export {index} has a size but no offset")));
        };
        let start = offset as usize;
        let end = match start.checked_add(entry.serial_size as usize) {
            Some(end) => end,
            None => return Err(layout(format!("export {index} serial span overflows"))),
        };
        if start < names_end || end > import_offset {
            return Err(layout(format!(
                "export {index} serial span [{start}, {end}) is outside payload region \
                 [{names_end}, {import_offset})"
            )));
        }
        spans.push((start, end, index));
    }

    spans.sort_unstable();
    let mut expected = names_end;
    for &(start, end, index) in &spans {
        if start != expected {
            let relation = if start < expected {
                "overlaps"
            } else {
                "leaves a gap before"
            };
            return Err(layout(format!(
                "export {index} serial span {relation} payload offset {expected}"
            )));
        }
        expected = end;
    }
    if expected != import_offset {
        return Err(layout(format!(
            "export serial spans end at {expected}, expected import offset {import_offset}"
        )));
    }
    check_offset(summary.export_offset, "export offset", file_len)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Node {
    Import(usize),
    Export(usize),
}

fn ref_target(reference: i32, export_count: usize, import_count: usize) -> Option<Node> {
    if reference == 0 {
        None
    } else if reference > 0 {
        debug_assert!(reference as usize <= export_count);
        Some(Node::Export(reference as usize - 1))
    } else {
        debug_assert!((-reference) as usize <= import_count);
        Some(Node::Import((-reference) as usize - 1))
    }
}

fn detect_outer_cycles(imports: &[ImportEntry], exports: &[ExportEntry]) -> PackageResult<()> {
    let walk_has_cycle = |start: Node| -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut current = Some(start);
        while let Some(node) = current {
            if !seen.insert(node) {
                return true;
            }
            current = match node {
                Node::Import(index) => {
                    ref_target(imports[index].outer_ref, exports.len(), imports.len())
                }
                Node::Export(index) => {
                    ref_target(exports[index].outer_ref, exports.len(), imports.len())
                }
            };
        }
        false
    };

    if (0..imports.len()).any(|index| walk_has_cycle(Node::Import(index)))
        || (0..exports.len()).any(|index| walk_has_cycle(Node::Export(index)))
    {
        return Err(PackageError::CycleInOuterRefs);
    }
    Ok(())
}

/// Serialize `archive` onto `out`, reproducing the original file bytes
/// exactly for any archive produced by [`read_package`].
pub fn write_package(archive: &PackageArchive, out: &mut Vec<u8>) -> Result<(), PackageError> {
    let summary = &archive.summary;

    // Layout preconditions established by the reader: the header gap fills
    // exactly up to name_offset, and v>=68 files have no gap at all.
    let mut staged = Vec::new();
    summary.write(&mut staged);
    let header_end = staged.len();
    let name_offset = summary.name_offset as usize;
    if header_end != name_offset {
        return Err(layout(format!(
            "serialized summary ends at {header_end}, expected name offset {name_offset}"
        )));
    }
    out.extend_from_slice(&staged);

    let pre_64 = summary.pre_64_names();
    for entry in &archive.names {
        tables::write_name_entry(out, entry, pre_64);
    }
    if out.len() != archive.names_end {
        return Err(layout(format!(
            "re-serialized name table ends at {}, expected {}",
            out.len(),
            archive.names_end
        )));
    }

    out.extend_from_slice(&archive.payload_region);
    if out.len() != summary.import_offset as usize {
        return Err(layout(format!(
            "re-serialized payload region ends at {}, expected import offset {}",
            out.len(),
            summary.import_offset
        )));
    }

    for entry in &archive.imports {
        tables::write_import_entry(out, entry);
    }
    if out.len() != summary.export_offset as usize {
        return Err(layout(format!(
            "re-serialized import table ends at {}, expected export offset {}",
            out.len(),
            summary.export_offset
        )));
    }

    for entry in &archive.exports {
        tables::write_export_entry(out, entry);
    }
    Ok(())
}

/// Convenience: read then write in one step, returning the re-serialized
/// bytes (used by round-trip tests).
pub fn round_trip(bytes: &[u8]) -> Result<Vec<u8>, PackageError> {
    let archive = read_package(bytes)?;
    let mut out = Vec::with_capacity(bytes.len());
    write_package(&archive, &mut out)?;
    Ok(out)
}
