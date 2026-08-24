//! Regression gate for the retail datadir (`Data/Retail`): its `HGame.u`
//! fork declares cross-package superclasses (`HGame.QuidArmor extends
//! HPModels.HPMeshActor`) and declares `GetCurrentKeyState` at slot 330 in
//! script, which must not collide with the engine-side intrinsic pin.
//! Skips when the retail root is absent.

use std::path::PathBuf;

fn retail_root() -> Option<PathBuf> {
    let root = PathBuf::from("/Users/mike/Library/Application Support/Harry Potter 2/Data/Retail");
    root.join("System/Core.u").is_file().then_some(root)
}

#[test]
fn retail_world_loads_and_binds() {
    let Some(root) = retail_root() else {
        eprintln!("retail datadir absent; skipping");
        return;
    };
    let world = hp_uobject::bootstrap::World::load(&root).expect("retail bootstrap");

    // QuidArmor resolves as a class whose chain crosses into HPModels.
    let quid_armor = world
        .arena
        .find_by_path("HGame.QuidArmor")
        .expect("HGame.QuidArmor missing");
    match &world.arena.get(quid_armor).expect("QuidArmor object").data {
        hp_uobject::arena::ObjectData::Class(class) => {
            let mesh_actor = class.super_class.expect("QuidArmor superclass unresolved");
            assert_eq!(
                world.arena.path_of(mesh_actor).as_deref(),
                Ok("HPModels.HPMeshActor"),
                "QuidArmor superclass is not HPModels.HPMeshActor"
            );
        }
        other => panic!("HGame.QuidArmor payload is not a Class: {other:?}"),
    }
    let core_object = world
        .arena
        .find_by_path("Core.Object")
        .expect("Core.Object missing");
    assert!(
        world
            .arena
            .class_is_a(quid_armor, core_object)
            .expect("class_is_a"),
        "HGame.QuidArmor does not descend from Core.Object"
    );

    hp_uobject::bind::verify_inheritance_chains(&world.arena).expect("inheritance chains hold");
    // Re-binding reports zero residuals across the retail package set.
    // Walk packages in load order — iterating the World's HashMap directly
    // would make this audit per-process nondeterministic.
    let mut exported_ids = Vec::new();
    for name in hp_uobject::bootstrap::PACKAGE_LOAD_ORDER {
        if let Some(ids) = world.exports.get(name) {
            exported_ids.extend(ids.iter().copied());
        }
    }
    let mut fresh = hp_uobject::natives::NativeRegistry::new();
    let report =
        hp_uobject::bind::bind_world(&world.arena, &mut fresh, &exported_ids).expect("re-bind");
    println!(
        "retail bind report: {} classes bound, {} unbound exports, \
         group bindings {:?}, {} ungrouped natives",
        report.classes_bound,
        report.unbound_classes.len(),
        report.group_bindings,
        report.ungrouped_natives.len()
    );
    assert!(
        report.unbound_classes.is_empty(),
        "retail untyped exports: {:?}",
        report.unbound_classes
    );
    assert!(
        report.ungrouped_natives.is_empty(),
        "retail unbound natives: {:?}",
        report.ungrouped_natives
    );

    // Script-declared GetCurrentKeyState wins slot 330 over the pin; oracle
    // slots all dispatchable.
    for slot in [112u16, 113, 256, 330] {
        assert!(
            world.registry.is_registered(slot),
            "retail oracle native slot {slot} is unbound"
        );
    }
}
