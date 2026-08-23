//! Level bootstrap, actor projection, tick orchestration, save
//! orchestration, camera/projection math, seeded determinism RNG policy.

/// Day-one identity marker; replaced by simulation tests from Phase 3.
pub const CRATE_NAME: &str = "hp-engine";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-engine");
    }
}
