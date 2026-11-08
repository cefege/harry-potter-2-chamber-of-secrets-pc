//! Record-level replay of `UGameEngine::SaveGame` (UnGame.cpp 2255–2308 at oracle 7ddb1d3).
//!
//! Drives `hp2_engine::game_save::save_game` through every case of the `save` family in
//! `Build/game_fixture_harness/game_harness.cpp` (`run_save`) and compares the full record
//! sequence against `fixtures/game_save.txt`.
//!
//! Not covered: `ExportTravel`, `BuildServerMasterMap`, `ConsumeLoadSlot`, and
//! `FormatLoadGameURL`. They share `Hp2GameSaveServices` but are not SaveGame.

#[allow(dead_code)]
#[path = "game_engine/mock.rs"]
mod mock;
#[path = "game_engine/save_mock.rs"]
mod save_mock;

use hp2_engine::game_save;
use mock::*;
use save_mock::{Wiring, services, wire};

const EXPECTED: &str = include_str!("fixtures/game_save.txt");
const EXPECTED_CASES: usize = 9;

fn case(pos: i32, pkg: i32, hub: i32, exist: i32, brush: bool, mover: bool) {
    let label = format!(
        "pos={pos} pkg={pkg} hub={hub} exist={exist} brush={} mover={}",
        i32::from(brush),
        i32::from(mover)
    );
    wire(Wiring {
        pkg_ret: pkg,
        exist,
        hub,
        brush,
        mover,
    });
    begin_family("save", &label);
    let s = services();
    // SAFETY: `engine()` is the live mock engine. SaveGame reaches the engine only through
    // `s`, and every service validates the handles it receives.
    unsafe { game_save::save_game(&s, engine(), pos) }.expect("native save replay succeeds");
}

fn run() {
    case(0, 1, 0, 0, false, false);
    case(1, 1, 1, 0, false, false);
    case(2, 1, 2, 2, false, false);
    case(3, 0, 1, 1, false, false);
    case(4, 1, 0, 0, true, false);
    case(5, 1, 0, 0, false, true);
    case(6, 1, 3, 1, true, true);
    case(7, 0, 0, 0, true, true);
    case(8, 1, 1, 3, false, false);
}

#[test]
fn game_save_replay_matches_oracle() {
    run();
    let out = take_output();

    let cases = out.lines().filter(|l| l.starts_with("case save ")).count();
    assert_eq!(cases, EXPECTED_CASES, "save case count");

    if out != EXPECTED {
        let (n, got, want) = out
            .lines()
            .zip(EXPECTED.lines())
            .enumerate()
            .find(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i + 1, a, b))
            .unwrap_or((0, "<length mismatch>", "<length mismatch>"));
        panic!(
            "save replay diverges from oracle at record {n}:\n  rust:   {got}\n  oracle: {want}"
        );
    }
}
