//! Loud, reason-coded errors for the launcher/app-shell slice.
//!
//! Every rejection carries a `Docs/REASON_CODES.md` `app.*` reason code so
//! callers (and crash records) can branch on stable identifiers instead of
//! prose.

use std::fmt;

/// An operation failure with its catalogued reason code attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    reason_code: &'static str,
    message: String,
}

impl AppError {
    pub fn new(reason_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason_code,
            message: message.into(),
        }
    }

    /// Stable dotted code registered in `Docs/REASON_CODES.md` (`app.*`).
    pub fn reason_code(&self) -> &'static str {
        self.reason_code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.reason_code, self.message)
    }
}

impl std::error::Error for AppError {}

/// Crate-wide result alias over [`AppError`].
pub type Result<T, E = AppError> = std::result::Result<T, E>;
