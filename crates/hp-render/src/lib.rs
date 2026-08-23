//! wgpu pipelines, texture/mesh managers, bitmap canvas, screenshot and
//! frame capture.

/// Day-one identity marker; replaced by renderer tests from Phase 4.
pub const CRATE_NAME: &str = "hp-render";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-render");
    }
}
