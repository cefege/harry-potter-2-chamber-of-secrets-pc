//! Reason-coded failures.
//!
//! Every loud failure carries a dotted lowercase reason code per
//! `Docs/REASON_CODES.md`; codes introduced by this crate live under the
//! `script.*` / `bind.*` / `native.*` domains.

use std::fmt;

/// A failure with a machine-greppable reason code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fail {
    pub reason_code: &'static str,
    pub message: String,
}

impl Fail {
    pub fn new(reason_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason_code,
            message: message.into(),
        }
    }
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.reason_code, self.message)
    }
}

impl std::error::Error for Fail {}

impl From<crate::name::SeedError> for Fail {
    fn from(err: crate::name::SeedError) -> Self {
        match err {
            crate::name::SeedError::IndexMismatch {
                text,
                expected,
                found,
            } => Self::new(
                "bind.name_seed_mismatch",
                format!("name {text:?} seeded at index {found}, expected {expected}"),
            ),
        }
    }
}

impl From<hp_format::package79::PackageError> for Fail {
    fn from(err: hp_format::package79::PackageError) -> Self {
        Self::new("bind.package_parse", format!("package parse failed: {err}"))
    }
}

pub type Result<T> = std::result::Result<T, Fail>;
