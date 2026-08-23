//! UE1 INI parse/merge/write with case-insensitive sections and keys.

/// Day-one identity marker; replaced by config-contract tests from Phase 1.
pub const CRATE_NAME: &str = "hp-ini";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-ini");
    }
}
