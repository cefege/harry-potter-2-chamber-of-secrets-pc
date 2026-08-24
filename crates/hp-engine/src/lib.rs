//! Level bootstrap, actor projection, tick orchestration, save
//! orchestration (byte-exact save reader/writer and native repair), camera
//! projection math, seeded determinism RNG policy.

pub mod console;
pub mod error;
pub mod input;
pub mod level;
pub mod rng;
pub mod save;
pub mod sim;

/// Day-one identity marker; replaced by simulation tests from Phase 3.
pub const CRATE_NAME: &str = "hp-engine";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-engine");
    }
}
