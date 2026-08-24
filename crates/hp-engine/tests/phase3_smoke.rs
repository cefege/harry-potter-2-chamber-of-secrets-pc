//! Phase 3 acceptance: three-map headless smoke plus determinism
//! double-run evidence for Docs/RNG_TICK_DIVERGENCE.md.

use std::path::{Path, PathBuf};

use hp_engine::input::InputScript;
use hp_engine::rng::DEFAULT_SEED;
use hp_engine::sim::{Engine, EngineOptions};
use hp_ini::load_pair;
use hp_uobject::snapshot::snapshot_world;

fn data_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal")
}

fn root_present(root: &Path) -> bool {
    root.join("System/Core.u").is_file()
}

/// Run the smoke pipeline exactly as `hp2rs` does and return the summary
/// line plus the arena snapshot hash.
fn run_map(map_token: &str) -> (String, [u8; 32], u64, u64) {
    let root = data_root();
    let ini = load_pair(&root.join("System/Default.ini"), None).expect("ini");
    let mut engine = Engine::bootstrap(
        &root,
        ini,
        EngineOptions {
            rng_seed: DEFAULT_SEED,
            fixed_dt: Some(1.0 / 30.0),
            editor_mode: false,
        },
    )
    .expect("bootstrap");
    engine.load_map(map_token).expect("map loads");
    // The fixture script's settle prefix keeps the run deterministic.
    let script = InputScript::parse("tick 0.033333335\nidle 8\npress K\nhold W 40\n").unwrap();
    engine
        .run_script_ticks(
            &script,
            120usize.saturating_sub(script.frames().len()),
            |_, _| {},
        )
        .expect("ticks complete");
    let counters = engine.counters();
    (
        format!(
            "ticks={} input={} console={} deferred={}",
            counters.ticks,
            counters.input_events,
            counters.console_commands,
            counters.script_deferrals
        ),
        snapshot_world(&engine.world.arena),
        counters.ticks,
        counters.script_deferrals,
    )
}

#[test]
fn three_gate_maps_smoke_clean() {
    let root = data_root();
    if !root_present(&root) {
        eprintln!("data-prototype root absent; skipping");
        return;
    }
    for token in [
        "..\\Maps\\PrivetDr.unr",
        "..\\Maps\\Entry.unr",
        "..\\Maps\\Ch2Skurge.unr",
    ] {
        let (summary, _, ticks, _) = run_map(token);
        assert_eq!(ticks, 120, "{token} must tick 120 times");
        eprintln!("{token}: {summary}");
    }
}

#[test]
fn determinism_double_run_identical() {
    let root = data_root();
    if !root_present(&root) {
        eprintln!("data-prototype root absent; skipping");
        return;
    }
    let (summary_a, hash_a, _, deferrals_a) = run_map("..\\Maps\\Entry.unr");
    let (summary_b, hash_b, _, deferrals_b) = run_map("..\\Maps\\Entry.unr");
    assert_eq!(summary_a, summary_b);
    assert_eq!(deferrals_a, deferrals_b);
    assert_eq!(hash_a, hash_b, "two identical runs must hash identically");
    eprintln!("entry snapshot sha256 {:02x?}", hash_a);
}
