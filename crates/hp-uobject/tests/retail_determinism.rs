//! Cross-process bind determinism gate for the retail datadir.
//!
//! Bootstrap must bind identically in EVERY process: the same packaged
//! binary once produced a clean run directly and a `QuidArmor` class_cycle
//! plus slot-330 duplicate under `game_test` isolation, because the export
//! audit set was walked in per-process HashMap order. This test spawns ten
//! separate child processes (the test binary re-invoked in child mode),
//! each bootstrapping the retail `World`, and requires every one to report
//! zero unbound classes and zero duplicate slots. Skips when the retail
//! root is absent.

use std::path::{Path, PathBuf};

fn retail_root() -> Option<PathBuf> {
    let root = PathBuf::from("/Users/mike/Library/Application Support/Harry Potter 2/Data/Retail");
    root.join("System/Core.u").is_file().then_some(root)
}

/// The child half: bootstrap, then print one machine-checkable verdict.
/// Exits the process so the parent's test harness never sees it as a test.
fn run_as_child(root: &Path) -> ! {
    let verdict = match hp_uobject::bootstrap::World::load(root) {
        Ok(world) => {
            // Zero unbound across every package, walked in load order.
            let mut exported_ids = Vec::new();
            for name in hp_uobject::bootstrap::PACKAGE_LOAD_ORDER {
                if let Some(ids) = world.exports.get(name) {
                    exported_ids.extend(ids.iter().copied());
                }
            }
            let mut registry = hp_uobject::natives::NativeRegistry::new();
            match hp_uobject::bind::bind_world(&world.arena, &mut registry, &exported_ids) {
                Ok(report) => {
                    let unbound = report.unbound_classes.len() + report.ungrouped_natives.len();
                    // QuidArmor's cross-package superclass resolves (the
                    // observed class_cycle case).
                    let quidarmor = world.arena.find_by_path("HGame.QuidArmor").is_some();
                    // Slot 330 owned exactly once (the observed duplicate-
                    // slot case): script declaration beats the pin cleanly.
                    let slot330 = world.registry.get(330).is_ok();
                    format!("DETERMINISM unbound={unbound} quidarmor={quidarmor} slot330={slot330}")
                }
                Err(error) => format!("DETERMINISM error={error}"),
            }
        }
        Err(error) => format!("DETERMINISM error={error}"),
    };
    println!("{verdict}");
    if verdict == "DETERMINISM unbound=0 quidarmor=true slot330=true" {
        std::process::exit(0);
    }
    std::process::exit(1);
}

#[test]
fn child_entry() {
    if let Ok(root) = std::env::var("HP_DETERMINISM_CHILD") {
        run_as_child(&PathBuf::from(root));
    }
}

#[test]
fn retail_bootstrap_is_deterministic_across_processes() {
    let Some(root) = retail_root() else {
        eprintln!("retail datadir absent; skipping");
        return;
    };

    let self_exe = std::env::current_exe().expect("test binary path");
    let expected = "DETERMINISM unbound=0 quidarmor=true slot330=true";

    for round in 0..10 {
        let output = std::process::Command::new(&self_exe)
            .args(["child_entry", "--exact", "--nocapture"])
            .env("HP_DETERMINISM_CHILD", &root)
            .output()
            .expect("spawn determinism probe child");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().find(|l| l.starts_with("DETERMINISM "));
        assert_eq!(
            line,
            Some(expected),
            "round {round}: bind outcome diverged; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
