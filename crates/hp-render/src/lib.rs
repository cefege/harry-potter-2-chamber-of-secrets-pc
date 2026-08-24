//! hp-render — wgpu renderer for the Rust engine rewrite.
//!
//! Milestone layout:
//! - [`device`]: headless GPU context + BGRA8888/24-bit-depth offscreen target
//! - [`math`]: projection FOV / clip-edge classification / console UI scale
//! - [`mesh_gpu`], [`pipelines`]: surface-class draw pipelines over
//!   `hp_format::mesh` readers
//! - [`textures`]: counted texture manager (`gl_textures_*` protocol names)
//! - [`canvas`]: bitmap glyph-quad canvas over `hp_format::font::GlyphAtlas`
//! - [`window`]: on-window swapchain target (interactive mode)
//!
pub mod block_on;
pub mod canvas;
pub mod capture;
pub mod device;
pub mod error;
pub mod math;
pub mod mesh_gpu;
pub mod pipelines;
pub mod textures;
pub mod window;

pub use capture::{CaptureMetaEntry, FrameCapture};
pub use device::{GpuContext, OffscreenTarget};
pub use error::{RenderError, ResourceLeakError};
/// Re-export so downstream engine crates can name GPU types without taking
/// a direct wgpu dependency (single version authority stays here).
pub use wgpu;
pub use window::{WindowFrame, WindowTarget};

/// Reason-code prefix for every loud renderer failure
/// (see Docs/REASON_CODES.md, `renderer.*` domain).
pub const REASON_DOMAIN: &str = "renderer";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::REASON_DOMAIN, "renderer");
    }
}
