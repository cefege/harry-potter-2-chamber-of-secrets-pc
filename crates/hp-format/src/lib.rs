//! Package79 archive read/write, compact index, FName/FString byte forms,
//! tagged properties, EA-XA decoder, DXT1 decode, mesh structs, font glyph
//! blits.

/// Day-one identity marker; replaced by format-contract tests from Phase 1.
pub const CRATE_NAME: &str = "hp-format";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-format");
    }
}
