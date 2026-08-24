//! Loud failure taxonomy for the package79 reader/writer.
//!
//! Every variant's `Display` starts with a reason-code token from
//! `Docs/REASON_CODES.md`; structural faults use `package.structure`.

use std::fmt;

/// Everything that can go wrong while decoding or re-encoding a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    /// Leading tag is not the Unreal package tag `0x9E2A83C1`.
    BadTag { got: u32 },
    /// FileVersion/LicenseeVersion pair outside the supported window
    /// (FileVersion 60..=79 with LicenseeVersion 0).
    UnsupportedVersion { got: i32 },
    /// Ran past the end of the input.
    Truncated { need: usize, got: usize },
    /// Compact-index continuation ran longer than five bytes.
    BadCompactIndex,
    /// Encoded compact index does not match its unique shortest form.
    NonCanonicalCompactIndex,
    /// Decoded compact-index magnitude falls outside int32.
    CompactIndexOutOfRange,
    /// Malformed FString (missing/duplicated NUL, invalid UTF-16, empty name).
    BadString,
    /// A name-table index is outside the parsed name count.
    BadNameIndex { index: i32, count: usize },
    /// An import/export object reference is out of range.
    BadObjectRef { reference: i32 },
    /// Summary/table/region invariant violated (counts disagree, regions
    /// overlap, payloads do not tile, unsupported pre-68 layout, ...).
    BadLayout { detail: String },
    /// Function export terminal fields match zero or several layouts.
    AmbiguousFunctionTerminal { candidates: usize },
    /// Import/export outer chain contains a cycle.
    CycleInOuterRefs,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadTag { got } => write!(
                f,
                "package.structure: bad package tag 0x{got:08x}; expected 0x9e2a83c1"
            ),
            Self::UnsupportedVersion { got } => write!(
                f,
                "package.structure: unsupported version {got}; \
                 supported FileVersion 60..=79 with LicenseeVersion 0"
            ),
            Self::Truncated { need, got } => write!(
                f,
                "package.structure: truncated input: need {need} byte(s), have {got}"
            ),
            Self::BadCompactIndex => {
                f.write_str("package.structure: compact index exceeds five bytes")
            }
            Self::NonCanonicalCompactIndex => {
                f.write_str("package.structure: non-canonical compact index")
            }
            Self::CompactIndexOutOfRange => {
                f.write_str("package.structure: compact index is outside int32")
            }
            Self::BadString => f.write_str("package.structure: malformed FString"),
            Self::BadNameIndex { index, count } => write!(
                f,
                "package.structure: name index {index} is outside name table count {count}"
            ),
            Self::BadObjectRef { reference } => write!(
                f,
                "package.structure: object reference {reference} is out of range"
            ),
            Self::BadLayout { detail } => write!(f, "package.structure: {detail}"),
            Self::AmbiguousFunctionTerminal { candidates } => write!(
                f,
                "package.structure: Function export has {candidates} structurally \
                 consistent terminal layouts; expected one"
            ),
            Self::CycleInOuterRefs => {
                f.write_str("package.structure: cycle in import/export outer references")
            }
        }
    }
}

impl std::error::Error for PackageError {}

/// Shorthand result type for this module.
pub type PackageResult<T> = Result<T, PackageError>;

/// Convenience constructor for [`PackageError::BadLayout`].
pub(crate) fn layout(detail: impl Into<String>) -> PackageError {
    PackageError::BadLayout {
        detail: detail.into(),
    }
}
