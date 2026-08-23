//! winit event loop ownership, CLI parsing, launcher profile store, folder
//! picker, input normalization, `<HP2_RES>` marker emission.

/// Day-one identity marker; replaced by launcher/input tests from Phase 5.
pub const CRATE_NAME: &str = "hp-app";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-app");
    }
}
