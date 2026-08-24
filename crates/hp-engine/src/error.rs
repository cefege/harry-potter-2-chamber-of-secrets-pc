//! Reason-coded engine failures (`engine.*` family, catalogued in
//! `Docs/REASON_CODES.md`).

use std::fmt;

/// A failure carrying a machine-greppable reason code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError {
    pub reason_code: &'static str,
    pub message: String,
}

impl EngineError {
    pub fn new(reason_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason_code,
            message: message.into(),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.reason_code, self.message)
    }
}

impl std::error::Error for EngineError {}

impl From<hp_uobject::error::Fail> for EngineError {
    fn from(fail: hp_uobject::error::Fail) -> Self {
        Self::new(fail.reason_code, fail.message)
    }
}

impl From<hp_format::package79::PackageError> for EngineError {
    fn from(err: hp_format::package79::PackageError) -> Self {
        Self::new("engine.map_parse", err.to_string())
    }
}

impl From<std::io::Error> for EngineError {
    fn from(err: std::io::Error) -> Self {
        Self::new("engine.io", err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;
