//! Loud, reason-coded renderer errors.
//!
//! Every failure carries a dotted lowercase reason code in the
//! `renderer.*` domain so harness greps and reports can classify it
//! without parsing prose (Docs/REASON_CODES.md convention).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// No usable GPU adapter (Metal on macOS) could be acquired.
    AdapterUnavailable { backend: &'static str },
    /// Logical device or queue creation failed.
    DeviceRequestFailed { detail: String },
    /// A GPU operation rejected its parameters.
    InvalidOperation {
        reason_code: &'static str,
        detail: String,
    },
    /// Frame capture could not encode/write its artifacts.
    CaptureWriteFailed { path: String, detail: String },
}

impl RenderError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            RenderError::AdapterUnavailable { .. } => "renderer.adapter_unavailable",
            RenderError::DeviceRequestFailed { .. } => "renderer.device_request_failed",
            RenderError::InvalidOperation { reason_code, .. } => reason_code,
            RenderError::CaptureWriteFailed { .. } => "renderer.capture_write_failed",
        }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::AdapterUnavailable { backend } => write!(
                f,
                "[{}] no {backend} adapter available; cannot render",
                self.reason_code()
            ),
            RenderError::DeviceRequestFailed { detail } => {
                write!(
                    f,
                    "[{}] device request failed: {detail}",
                    self.reason_code()
                )
            }
            RenderError::InvalidOperation { detail, .. } => {
                write!(f, "[{}] {detail}", self.reason_code())
            }
            RenderError::CaptureWriteFailed { path, detail } => {
                write!(f, "[{}] writing {path}: {detail}", self.reason_code())
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// Texture-manager imbalance at shutdown: creations must equal destructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLeakError {
    pub created: u64,
    pub destroyed: u64,
    pub live: usize,
}

impl ResourceLeakError {
    pub const REASON_CODE: &'static str = "resources.leak";
}

impl fmt::Display for ResourceLeakError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] texture leak: created={} destroyed={} still-live={}",
            Self::REASON_CODE,
            self.created,
            self.destroyed,
            self.live
        )
    }
}

impl std::error::Error for ResourceLeakError {}
