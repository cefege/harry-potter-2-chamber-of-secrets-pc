//! rodio output graph, Ogg-loop and XA stream types.

/// Day-one identity marker; replaced by audio-lifecycle tests from Phase 5.
pub const CRATE_NAME: &str = "hp-audio";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-audio");
    }
}
