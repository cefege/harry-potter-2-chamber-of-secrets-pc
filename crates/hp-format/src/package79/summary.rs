//! Package summary header (versions 60..=79): fixed fields, the GUID +
//! generation-table directory (FileVersion >= 68), and the pre-68 heritage
//! directory whose trailing header-gap bytes are preserved verbatim.
//!
//! Normative behavior source: `Build/package79_reference.py::parse_summary`.

use super::cursor::{
    ByteCursor, LICENSEE_VERSION, PACKAGE_MAX_VERSION, PACKAGE_MIN_VERSION, PACKAGE_TAG,
};
use super::error::{PackageError, PackageResult, layout};

/// GUID carried by FileVersion >= 68 summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub d: u32,
}

/// One (export_count, name_count) pair of the generation table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationEntry {
    pub export_count: i32,
    pub name_count: i32,
}

/// Post-summary table directory, keyed on FileVersion exactly like the
/// reference reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableDirectory {
    /// FileVersion >= 68: 16-byte GUID followed by the generation table.
    Generations {
        guid: Guid,
        entries: Vec<GenerationEntry>,
    },
    /// FileVersion < 68: HeritageCount/HeritageOffset pair where the GUID
    /// would sit; the heritage table itself lives in the header gap.
    Heritage { count: i32, offset: i32 },
}

/// Parsed package summary header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSummary {
    pub tag: u32,
    /// Raw 32-bit word; `version` = low 16 bits, `licensee` = high 16 bits.
    pub version_word: u32,
    pub version: i32,
    pub licensee: i32,
    pub package_flags: u32,
    pub name_count: i32,
    pub name_offset: i32,
    pub export_count: i32,
    pub export_offset: i32,
    pub import_count: i32,
    pub import_offset: i32,
    pub tables: TableDirectory,
    /// Byte offset just past the summary proper (before any header gap).
    pub header_size: usize,
    /// Verbatim bytes between `header_size` and the name table. Pre-v68 files
    /// park their heritage tables here; the writer reproduces these bytes
    /// unchanged. Always empty for FileVersion >= 68.
    pub header_gap: Vec<u8>,
}

pub(crate) fn check_count(value: i32, label: &str) -> PackageResult<()> {
    if value < 0 {
        return Err(layout(format!("negative {label}: {value}")));
    }
    Ok(())
}

pub(crate) fn check_offset(value: i32, label: &str, file_len: usize) -> PackageResult<()> {
    if value < 0 {
        return Err(layout(format!(
            "{label} {value} is outside file size {file_len}"
        )));
    }
    if value as usize > file_len {
        return Err(layout(format!(
            "{label} {value} is outside file size {file_len}"
        )));
    }
    Ok(())
}

/// Parse and validate the summary header starting at offset 0 of `data`.
pub fn parse_summary(data: &[u8]) -> PackageResult<PackageSummary> {
    let file_len = data.len();
    let mut cur = ByteCursor::new(data);

    let tag = cur.u32()?;
    let version_word = cur.u32()?;
    let package_flags = cur.u32()?;
    let name_count = cur.i32()?;
    let name_offset = cur.i32()?;
    let export_count = cur.i32()?;
    let export_offset = cur.i32()?;
    let import_count = cur.i32()?;
    let import_offset = cur.i32()?;

    let version = i32::from((version_word & 0xFFFF) as u16);
    let licensee = i32::from(((version_word >> 16) & 0xFFFF) as u16);

    if tag != PACKAGE_TAG {
        return Err(PackageError::BadTag { got: tag });
    }
    if !(PACKAGE_MIN_VERSION..=PACKAGE_MAX_VERSION).contains(&version)
        || licensee != LICENSEE_VERSION
    {
        return Err(PackageError::UnsupportedVersion { got: version });
    }

    let tables = if version >= 68 {
        let guid = Guid {
            a: cur.u32()?,
            b: cur.u32()?,
            c: cur.u32()?,
            d: cur.u32()?,
        };
        let generation_count = cur.i32()?;
        check_count(generation_count, "generation count")?;
        if generation_count == 0 {
            return Err(layout("package has no generations"));
        }
        if generation_count as usize > (file_len - cur.position()) / 8 {
            return Err(layout("generation table is truncated"));
        }
        let mut entries = Vec::with_capacity(generation_count as usize);
        for index in 0..generation_count as usize {
            let export_count = cur.i32()?;
            let name_count = cur.i32()?;
            check_count(export_count, "generation export count")
                .map_err(|_| layout(format!("generation {index} has a negative export count")))?;
            check_count(name_count, "generation name count")
                .map_err(|_| layout(format!("generation {index} has a negative name count")))?;
            entries.push(GenerationEntry {
                export_count,
                name_count,
            });
        }
        let latest = &entries[entries.len() - 1];
        if latest.export_count != export_count || latest.name_count != name_count {
            return Err(layout("latest generation counts disagree with summary"));
        }
        TableDirectory::Generations { guid, entries }
    } else {
        let heritage_count = cur.i32()?;
        let heritage_offset = cur.i32()?;
        check_count(heritage_count, "heritage count")?;
        check_offset(heritage_offset, "heritage offset", file_len)?;
        TableDirectory::Heritage {
            count: heritage_count,
            offset: heritage_offset,
        }
    };

    let header_size = cur.position();

    check_count(name_count, "name count")?;
    check_count(export_count, "export count")?;
    check_count(import_count, "import count")?;
    check_offset(name_offset, "name offset", file_len)?;
    check_offset(export_offset, "export offset", file_len)?;
    check_offset(import_offset, "import offset", file_len)?;

    let header_gap = if version >= 68 {
        if name_offset as usize != header_size {
            return Err(layout(format!(
                "name table begins at {name_offset}, not immediately after summary at {header_size}"
            )));
        }
        Vec::new()
    } else {
        if (name_offset as usize) < header_size {
            return Err(layout(format!(
                "name table begins at {name_offset}, before end of heritage summary at {header_size}"
            )));
        }
        data[header_size..name_offset as usize].to_vec()
    };

    Ok(PackageSummary {
        tag,
        version_word,
        version,
        licensee,
        package_flags,
        name_count,
        name_offset,
        export_count,
        export_offset,
        import_count,
        import_offset,
        tables,
        header_size,
        header_gap,
    })
}

impl PackageSummary {
    /// True when name entries use the pre-64 NUL-terminated ANSI form.
    pub fn pre_64_names(&self) -> bool {
        self.version < 64
    }

    /// Serialize the summary header (including the verbatim header gap for
    /// pre-v68 files) onto `out`.
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.tag.to_le_bytes());
        out.extend_from_slice(&self.version_word.to_le_bytes());
        out.extend_from_slice(&self.package_flags.to_le_bytes());
        for field in [
            self.name_count,
            self.name_offset,
            self.export_count,
            self.export_offset,
            self.import_count,
            self.import_offset,
        ] {
            out.extend_from_slice(&field.to_le_bytes());
        }
        match &self.tables {
            TableDirectory::Generations { guid, entries } => {
                for word in [guid.a, guid.b, guid.c, guid.d] {
                    out.extend_from_slice(&word.to_le_bytes());
                }
                out.extend_from_slice(&(entries.len() as i32).to_le_bytes());
                for entry in entries {
                    out.extend_from_slice(&entry.export_count.to_le_bytes());
                    out.extend_from_slice(&entry.name_count.to_le_bytes());
                }
            }
            TableDirectory::Heritage { count, offset } => {
                out.extend_from_slice(&count.to_le_bytes());
                out.extend_from_slice(&offset.to_le_bytes());
                out.extend_from_slice(&self.header_gap);
            }
        }
    }
}
