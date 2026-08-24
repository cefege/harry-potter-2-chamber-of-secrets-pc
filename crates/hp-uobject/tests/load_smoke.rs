//! Smoke gate: all three stock packages load into one arena.

use std::path::{Path, PathBuf};

fn data_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal")
}
fn root_present(root: &Path) -> bool {
    root.join("System/Core.u").is_file()
}

#[test]
fn loads_core_engine_hgame() {
    let root = data_root();
    if !root_present(&root) {
        eprintln!("data-prototype root absent; skipping");
        return;
    }
    let mut arena = hp_uobject::arena::ObjectArena::new();
    for name in ["Core", "Engine", "UWindow", "HPParticle", "HGame"] {
        let bytes = std::fs::read(root.join("System").join(format!("{name}.u"))).unwrap();
        let loaded = hp_uobject::loader::load_package(&mut arena, name, &bytes)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        println!(
            "{name}: root path = {}",
            arena.path_of(loaded.root).unwrap()
        );
    }
    println!("total objects: {}", arena.len());
}
