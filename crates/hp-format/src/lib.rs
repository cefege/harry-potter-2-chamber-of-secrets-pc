//! Package79 archive read/write, compact index, FName/FString byte forms,
//! tagged properties, EA-XA decoder, DXT1 decode, mesh structs, font glyph
//! blits.
//!
//! Module ownership during Phase 1 (do not cross-edit):
//! - `package79` — Track A (archive reader/writer, audit parity)
//! - `eaxa`, `dxt1`, `mesh`, `font` — Track B (codecs/config/mesh/font)

pub mod dxt1;
pub mod eaxa;
pub mod font;
pub mod mesh;
pub mod package79;

/// Day-one identity marker; replaced by format-contract tests from Phase 1.
pub const CRATE_NAME: &str = "hp-format";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-format");
    }
}
