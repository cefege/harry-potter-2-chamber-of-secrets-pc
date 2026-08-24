//! Phase 5 save slice: byte-exact save reader/writer and the native
//! `repair_save` entry point, built on the verified hp_format package79
//! codec (214/214 round-trip parity with `Build/package79_reference.py`).
//!
//! Saves *are* v79 packages, so decoding delegates to
//! [`hp_format::package79::read_package`]; this module adds the save-level
//! rejection taxonomy: every structural fault is a loud error whose dotted
//! reason code and invariant label are copied verbatim from the golden
//! corpus specification at `Tests/Fixtures/save-format-golden/MANIFEST.json`
//! (`Tests/SaveFormatTests.py` embeds the same table).

mod repair;
#[cfg(test)]
mod tests;

pub use repair::{RepairError, RepairReport, parse_save_slot, repair_save};

use hp_format::package79::{PackageArchive, PackageError};

/// Inclusive summary words shared by every supported FileVersion:
/// tag, version word, package flags, name count/offset,
/// export count/offset, import count/offset.
const SUMMARY_FIXED_BYTES: usize = 36;
/// Fixed summary extent for FileVersion >= 68: fixed words plus the
/// 16-byte GUID and the 4-byte generation count.
const V68_SUMMARY_FIXED_BYTES: usize = 56;

/// One labeled structural fault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFault {
    /// Dotted reason code, matching `reason_code` in the golden manifest.
    pub reason_code: &'static str,
    /// Invariant label, matching `invariant` in the golden manifest.
    pub invariant: &'static str,
    /// Human-readable diagnostic; must contain the manifest's
    /// `message_contains` fragment for the corresponding case.
    pub detail: String,
}

impl std::fmt::Display for SaveFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "save.package79 [{}] {}: {}",
            self.reason_code, self.invariant, self.detail
        )
    }
}

/// Everything that can go wrong while reading or writing a save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// Structural fault with a golden-manifest reason code.
    Fault(SaveFault),
    /// Package decoder fault outside the manifest's labeled cases; the
    /// wrapped display already carries the catalog's `package.structure`
    /// reason token.
    Package(PackageError),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fault(fault) => fault.fmt(f),
            Self::Package(error) => write!(f, "save.package79 passthrough: {error}"),
        }
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Package(error) => Some(error),
            Self::Fault(_) => None,
        }
    }
}

impl From<SaveFault> for SaveError {
    fn from(fault: SaveFault) -> Self {
        Self::Fault(fault)
    }
}

impl SaveError {
    /// The manifest reason code when this is a labeled structural fault.
    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Fault(fault) => Some(fault.reason_code),
            Self::Package(_) => None,
        }
    }

    /// The manifest invariant label when this is a labeled structural fault.
    pub fn invariant(&self) -> Option<&'static str> {
        match self {
            Self::Fault(fault) => Some(fault.invariant),
            Self::Package(_) => None,
        }
    }
}

/// Shorthand result type for the save reader/writer.
pub type SaveResult<T> = Result<T, SaveError>;

fn fault(
    reason_code: &'static str,
    invariant: &'static str,
    detail: impl Into<String>,
) -> SaveError {
    SaveError::Fault(SaveFault {
        reason_code,
        invariant,
        detail: detail.into(),
    })
}

/// Read one save file. Valid golden saves decode into a
/// [`hp_format::package79::PackageArchive`] that re-serializes byte-exactly;
/// corrupt mutants are rejected with exactly the reason codes the golden
/// corpus manifest demands.
pub fn read_save(data: &[u8]) -> SaveResult<PackageArchive> {
    preflight_summary(data)?;
    hp_format::package79::read_package(data).map_err(|error| classify(data, error))
}

/// Serialize a parsed save onto `out`, reproducing the source file bytes
/// exactly ([`read_save`] then [`write_save`] is an identity on anything
/// that decoded).
pub fn write_save(archive: &PackageArchive, out: &mut Vec<u8>) -> SaveResult<()> {
    hp_format::package79::write_package(archive, out).map_err(SaveError::Package)
}

/// Header checks that must fire before the generic package decoder so each
/// mutant gets its own manifest-labeled code instead of a combined
/// "unsupported version" or truncation diagnostic.
///
/// Byte offsets follow the v68+ summary layout: tag@0, version word@4,
/// name count@12, export count@20, GUID@36..52, generation count@52,
/// generation table@56..
fn preflight_summary(data: &[u8]) -> SaveResult<()> {
    require_summary_bytes(data, SUMMARY_FIXED_BYTES, "the fixed summary words")?;

    let tag = u32_at(data, 0);
    if tag != hp_format::package79::PACKAGE_TAG {
        return Err(fault(
            "header.bad_tag",
            "save.package79.header.tag",
            format!(
                "bad package tag 0x{tag:08x}; expected 0x{:08x}",
                hp_format::package79::PACKAGE_TAG
            ),
        ));
    }

    let version_word = u32_at(data, 4);
    let version = i32::from(version_word as u16);
    let licensee = i32::from((version_word >> 16) as u16);
    let min = hp_format::package79::PACKAGE_MIN_VERSION;
    let max = hp_format::package79::PACKAGE_MAX_VERSION;
    let licensee_expected = hp_format::package79::LICENSEE_VERSION;
    if !(min..=max).contains(&version) || licensee != licensee_expected {
        // Licensee mismatch wins attribution: the manifest distinguishes the
        // two mutants only by which field moved.
        let (code, invariant) = if licensee != licensee_expected {
            ("header.bad_licensee", "save.package79.header.licensee")
        } else {
            ("header.bad_version", "save.package79.header.version")
        };
        return Err(fault(
            code,
            invariant,
            format!(
                "unsupported version/licensee {version}/{licensee}; \
                 supported: FileVersion in [{min}, {max}] with LicenseeVersion {licensee_expected}"
            ),
        ));
    }

    if version >= 68 {
        require_summary_bytes(data, V68_SUMMARY_FIXED_BYTES, "GUID and generation count")?;
        let summary_name_count = i32_at(data, 12);
        let summary_export_count = i32_at(data, 20);
        let generation_count = i32_at(data, 52);
        if generation_count > 0 {
            let table_end = V68_SUMMARY_FIXED_BYTES
                + (generation_count as usize)
                    .checked_mul(8)
                    .expect("generation count fits usize");
            require_summary_bytes(data, table_end, "the generation table")?;
            let last = V68_SUMMARY_FIXED_BYTES + (generation_count as usize - 1) * 8;
            let latest_export_count = i32_at(data, last);
            let latest_name_count = i32_at(data, last + 4);
            if latest_export_count != summary_export_count
                || latest_name_count != summary_name_count
            {
                return Err(fault(
                    "header.generation_disagreement",
                    "save.package79.header.generation_consistency",
                    "latest generation counts disagree with summary",
                ));
            }
        }
        // generation_count <= 0 is left to the decoder, which rejects it
        // with its own loud layout diagnostic.
    }

    Ok(())
}

fn require_summary_bytes(data: &[u8], need: usize, what: &str) -> SaveResult<()> {
    if data.len() < need {
        Err(fault(
            "structure.truncated_summary",
            "save.package79.structure.summary_present",
            format!(
                "truncated package summary: need {need} byte(s) for {what}, have {}",
                data.len()
            ),
        ))
    } else {
        Ok(())
    }
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().expect("4 bytes"))
}

fn i32_at(data: &[u8], offset: usize) -> i32 {
    u32_at(data, offset) as i32
}

/// Attribute a decoder failure to its save-level manifest label. Only the
/// cases the golden corpus actually specifies get relabeled; everything
/// else passes through with the decoder's own `package.structure`
/// diagnostic intact.
fn classify(data: &[u8], error: PackageError) -> SaveError {
    match &error {
        PackageError::Truncated { need, got } => {
            // The decoder reads names, then imports, then exports; whichever
            // stage still fails to walk pins down which table was cut short.
            let truncated_table = match hp_format::package79::summary::parse_summary(data) {
                Err(_) => None,
                Ok(summary) => match hp_format::package79::tables::parse_names(data, &summary) {
                    Err(_) => None,
                    Ok((names, _)) => {
                        match hp_format::package79::tables::parse_imports(data, &summary, &names) {
                            Err(_) => None,
                            Ok(_) => Some(()),
                        }
                    }
                },
            };
            if truncated_table.is_some() {
                // Only an export-table cut reaches here labeled; name/import
                // truncation stays under the decoder's package.structure
                // token because no manifest case defines a finer label.
                return fault(
                    "structure.truncated_export_table",
                    "save.package79.structure.export_table_complete",
                    format!("truncated export table: need {need} byte(s), have {got}"),
                );
            }
            SaveError::Package(error)
        }
        PackageError::BadObjectRef { reference } => fault(
            "table.dangling_reference",
            "save.package79.table.reference_integrity",
            format!("object reference {reference} is out of range"),
        ),
        PackageError::BadLayout { detail } if detail.contains("is out of range") => fault(
            "table.dangling_reference",
            "save.package79.table.reference_integrity",
            detail.clone(),
        ),
        PackageError::BadLayout { detail } if detail.contains("disagree with summary") => fault(
            "header.generation_disagreement",
            "save.package79.header.generation_consistency",
            detail.clone(),
        ),
        _ => SaveError::Package(error),
    }
}
