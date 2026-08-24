//! Port of the oracle ctest `Tests/SpellRuntimeTests.cpp`
//! (`spell_runtime_contracts`) to the Rust object model.
//!
//! The C++ test boots the runtime with `InstallHP2NativeLookups`, loads
//! Engine.u and HGame.u as a load boundary, then checks the Harry
//! reflection contract: `HGame.Harry` static-loads as an `APlayerPawn`
//! subclass, carries thirteen named bool properties plus the object
//! property `BossTarget`, and `Engine.Mover` is findable afterwards.

use std::path::{Path, PathBuf};

fn data_root() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
    root.join("System/Core.u").is_file().then_some(root)
}

#[test]
fn spell_runtime_contracts() {
    let Some(root) = data_root() else {
        eprintln!("data-prototype root absent; skipping");
        return;
    };

    // TestStage "Engine.u-load": the engine package loads standalone.
    {
        let mut arena = hp_uobject::arena::ObjectArena::new();
        let bytes = std::fs::read(engine_path(&root)).expect("Engine.u readable");
        hp_uobject::loader::load_package(&mut arena, "Engine", &bytes)
            .expect("Engine package load failed");
    }

    // TestStage "HGame.u-load": the game package loads standalone.
    {
        let mut arena = hp_uobject::arena::ObjectArena::new();
        let bytes = std::fs::read(hgame_path(&root)).expect("HGame.u readable");
        hp_uobject::loader::load_package(&mut arena, "HGame", &bytes)
            .expect("HGame package load failed");
    }

    // Full boot: InstallHP2NativeLookups equivalent + all stock packages
    // + native binding.
    let world = hp_uobject::bootstrap::World::load(&root).expect("runtime bootstrap failed");

    // TestStage "Harry-reflection-contract": StaticLoadClass(APlayerPawn,
    // "HGame.Harry") resolves, so Harry must be a PlayerPawn descendant.
    let harry = world
        .arena
        .find_by_path("HGame.Harry")
        .expect("HGame.Harry class is unavailable after package load");
    for ancestor in [
        "Engine.PlayerPawn",
        "Engine.Pawn",
        "Engine.Actor",
        "Core.Object",
    ] {
        let id = world
            .arena
            .find_by_path(ancestor)
            .unwrap_or_else(|| panic!("{ancestor} unavailable"));
        assert!(
            world.arena.class_is_a(harry, id).expect("class_is_a"),
            "HGame.Harry is not a {ancestor} subclass"
        );
    }

    // Inheritance chains hold across every loaded class.
    hp_uobject::bind::verify_inheritance_chains(&world.arena).expect("inheritance chains hold");

    // FindField<UBoolProperty>(HarryClass, …) for each of the thirteen
    // contract bools, FindField<UObjectProperty>(HarryClass, "BossTarget"),
    // and StaticFindObject(UClass, ANY_PACKAGE, "Engine.Mover", 1) — all in
    // one loud reason-coded pass.
    hp_uobject::bind::verify_harry_contract(&world.arena).expect("Harry reflection contract");
}

fn engine_path(root: &Path) -> PathBuf {
    root.join("System/Engine.u")
}

fn hgame_path(root: &Path) -> PathBuf {
    root.join("System/HGame.u")
}
