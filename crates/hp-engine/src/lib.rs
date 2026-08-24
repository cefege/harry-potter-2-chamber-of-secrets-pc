//! Level bootstrap, actor projection, tick orchestration, save
//! orchestration (byte-exact save reader/writer and native repair), camera
//! projection math, seeded determinism RNG policy.

pub mod save;

/// Day-one identity marker; replaced by simulation tests from Phase 3.
pub const CRATE_NAME: &str = "hp-engine";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-engine");
    }
}
