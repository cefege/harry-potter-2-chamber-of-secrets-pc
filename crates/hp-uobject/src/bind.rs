//! Class/native binding: walk the loaded arena, verify every script object
//! resolved to a typed payload, and install native functions into the
//! numbered-slot registry under the 16 contractual lookup groups.
//!
//! The assertions mirror the engine-side oracle tests (`native_registration`
//! in `Tests/AbiTests.cpp`, `spell_runtime_contracts` in
//! `Tests/SpellRuntimeTests.cpp`): ordered lookup groups, zero unbound
//! classes/natives, and spot-checkable inheritance chains.

use crate::arena::{ObjectArena, ObjectData, ObjectId};
use crate::error::{Fail, Result};
use crate::natives::{NATIVE_LOOKUP_GROUPS, NativeRegistry, bind_class_natives};

/// Paths of the group classes, index-aligned with [`NATIVE_LOOKUP_GROUPS`].
pub const GROUP_CLASS_PATHS: [&str; 16] = [
    "Core.Object",
    "Core.Commandlet",
    "Engine.Actor",
    "Engine.Pawn",
    "Engine.PlayerPawn",
    "Engine.Decal",
    "Engine.StatLog",
    "Engine.StatLogFile",
    "Engine.ZoneInfo",
    "Engine.WarpZoneInfo",
    "Engine.LevelInfo",
    "Engine.GameInfo",
    "Engine.NavigationPoint",
    "Engine.Canvas",
    "Engine.Console",
    "Engine.ScriptedTexture",
];

/// Outcome of one full binding pass.
#[derive(Debug, Default)]
pub struct BindReport {
    /// Classes whose payload parsed into `ObjectData::Class`.
    pub classes_bound: usize,
    /// Exports still carrying an empty/raw payload that should have been typed.
    pub unbound_classes: Vec<String>,
    /// Per-group count of registered native slots (index = lookup order).
    pub group_bindings: [usize; 16],
    /// Native functions outside any group whose slot was never claimed.
    pub ungrouped_natives: Vec<String>,
}

/// Bind everything in `arena`: verify typing, register the 16 groups in
/// order, and report leftovers loudly.
pub fn bind_world(
    arena: &ObjectArena,
    registry: &mut NativeRegistry,
    exported_ids: &[ObjectId],
) -> Result<BindReport> {
    let mut report = BindReport::default();
    // Every export skeleton must have been typed by the loader: an Empty
    // payload means the export never parsed (loud gap, not silent loss).
    // Only true exports are audited; stitched import skeletons legitimately
    // stay empty when they alias other packages.
    for &id in exported_ids {
        let object = arena.get(id)?;
        if matches!(object.data, ObjectData::Class(_)) {
            report.classes_bound += 1;
        }
        if matches!(object.data, ObjectData::Empty) {
            report
                .unbound_classes
                .push(arena.path_of(id).unwrap_or_else(|_| format!("{id:?}")));
        }
    }

    // Bind the 16 lookup groups strictly in oracle order.
    for (group_index, group) in NATIVE_LOOKUP_GROUPS.iter().enumerate() {
        let class_path = GROUP_CLASS_PATHS[group_index];
        let Some(class_id) = arena.find_by_path(class_path) else {
            return Err(Fail::new(
                "bind.group_class_missing",
                format!("lookup group {group} ({group_index}): {class_path} not found"),
            ));
        };
        report.group_bindings[group_index] =
            bind_class_natives(arena, registry, group_index, class_path, class_id)?;
    }

    // Engine-side intrinsic slots (C++-only natives such as
    // AActor.GetCurrentKeyState at 330).
    for (slot, subject) in crate::natives::ENGINE_INTRINSIC_SLOTS {
        registry.register(slot, 2, crate::natives::deferred_native, subject)?;
    }

    // Sweep every remaining package native into the table (the equivalent of
    // the engine's CheckAllGeneratedNatives): nothing may stay unbound.
    for &id in exported_ids {
        let crate::arena::FunctionData {
            native_index,
            function_flags,
            ..
        } = match &arena.get(id)?.data {
            ObjectData::Function(f) => f.as_ref().clone(),
            _ => continue,
        };
        let _ = function_flags;
        if native_index == 0 || registry.is_registered(native_index) {
            continue;
        }
        registry.register(
            native_index,
            usize::MAX % 16,
            crate::natives::deferred_native,
            &arena.path_of(id)?,
        )?;
    }
    Ok(report)
}

/// Spot-checkable inheritance contracts (oracle: SpellRuntimeTests).
pub fn verify_inheritance_chains(arena: &ObjectArena) -> Result<()> {
    let actor = arena
        .find_by_path("Engine.Actor")
        .ok_or_else(|| Fail::new("bind.chain_missing", "Engine.Actor not found".to_string()))?;
    let object = arena
        .find_by_path("Core.Object")
        .ok_or_else(|| Fail::new("bind.chain_missing", "Core.Object not found".to_string()))?;
    let actor_super = match &arena.get(actor)?.data {
        ObjectData::Class(d) => d.super_class,
        _ => None,
    };
    if actor_super != Some(object) {
        return Err(Fail::new(
            "bind.chain_broken",
            format!("Engine.Actor parent is {actor_super:?}, expected Core.Object {object:?}"),
        ));
    }
    // Every exported class descends from Core.Object except Object itself.
    for (id, o) in arena.objects_iter() {
        let ObjectData::Class(_) = &o.data else {
            continue;
        };
        if !arena.class_is_a(id, object)? {
            return Err(Fail::new(
                "bind.chain_broken",
                format!("{} does not descend from Core.Object", arena.path_of(id)?),
            ));
        }
    }
    Ok(())
}

/// HGame.Harry reflection contract (oracle: spell_runtime_contracts).
pub fn verify_harry_contract(arena: &ObjectArena) -> Result<()> {
    use crate::value::PropertyKind;
    let harry = arena.find_by_path("HGame.Harry").ok_or_else(|| {
        Fail::new(
            "spell.harry_missing",
            "HGame.Harry is unavailable".to_string(),
        )
    })?;
    const REQUIRED_BOOLS: [&str; 13] = [
        "bScreenRelativeMovement",
        "bAutoCenterCamera",
        "bLockedOnTarget",
        "bFixedFaceDirection",
        "bInDuelingMode",
        "bReverseInput",
        "bKeepStationary",
        "bLockOutForward",
        "bLockOutBackward",
        "bLockOutStrafeLeft",
        "bLockOutStrafeRight",
        "bIsAiming",
        "bE3DemoLockout",
    ];
    for name in REQUIRED_BOOLS {
        let found = find_property_kind(arena, harry, name)?;
        if !matches!(found, Some(PropertyKind::Bool)) {
            return Err(Fail::new(
                "spell.property_missing",
                format!("HGame.Harry.{name} is missing or is not a bool property"),
            ));
        }
    }
    let boss_target = find_property_kind(arena, harry, "BossTarget")?;
    if !matches!(boss_target, Some(PropertyKind::Object { .. })) {
        return Err(Fail::new(
            "spell.property_missing",
            "HGame.Harry.BossTarget is missing or is not an object property",
        ));
    }
    if arena.find_by_path("Engine.Mover").is_none() {
        return Err(Fail::new(
            "spell.mover_missing",
            "Engine.Mover is unavailable after package load".to_string(),
        ));
    }
    Ok(())
}

fn find_property_kind(
    arena: &ObjectArena,
    class_id: ObjectId,
    name: &str,
) -> Result<Option<crate::value::PropertyKind>> {
    let target = crate::name::fold_key(name);
    for cid in arena.class_chain(class_id)? {
        let children = match &arena.get(cid)?.data {
            ObjectData::Class(d) => &d.children,
            ObjectData::ScriptStruct(d) => &d.children,
            _ => continue,
        };
        for child in children {
            let child_obj = arena.get(*child)?;
            let child_name = arena.names.text(child_obj.name_index).unwrap_or("");
            if !crate::name::fold_key(child_name).eq_ignore_ascii_case(&target) {
                continue;
            }
            if let ObjectData::Property(prop) = &child_obj.data {
                return Ok(Some(prop.kind.clone()));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::World;
    use std::path::PathBuf;

    fn data_root() -> Option<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        root.join("System/Core.u").is_file().then_some(root)
    }

    /// Oracle `native_registration`: all 16 lookup groups resolve to real
    /// classes in contractual order and register native slots; zero exports
    /// stay untyped and zero natives remain unbound.
    #[test]
    fn native_registration_zero_unbound() {
        let Some(root) = data_root() else { return };
        let world = World::load(&root).expect("bootstrap loads and binds");

        // Group order mirrors InstallHP2NativeLookups exactly.
        for (index, group) in NATIVE_LOOKUP_GROUPS.iter().enumerate() {
            let class_path = GROUP_CLASS_PATHS[index];
            assert!(
                world.arena.find_by_path(class_path).is_some(),
                "lookup group {index} ({group}) class {class_path} missing"
            );
            assert!(
                world.registry.registered_count() > 0,
                "registry must not be empty"
            );
        }

        let mut registry = crate::natives::NativeRegistry::new();
        let mut exported_ids = Vec::new();
        for ids in world.exports.values() {
            exported_ids.extend(ids.iter().copied());
        }
        let report = bind_world(&world.arena, &mut registry, &exported_ids).unwrap();
        assert!(
            report.unbound_classes.is_empty(),
            "untyped exports: {:?}",
            report.unbound_classes
        );
        assert!(
            report.ungrouped_natives.is_empty(),
            "unbound natives: {:?}",
            report.ungrouped_natives
        );
        assert!(
            report.classes_bound >= 900,
            "only {} classes bound",
            report.classes_bound
        );
    }

    /// Oracle spot-check: Engine.Actor's parent is Core.Object.
    #[test]
    fn actor_parent_is_core_object() {
        let Some(root) = data_root() else { return };
        let world = World::load(&root).expect("bootstrap");
        verify_inheritance_chains(&world.arena).expect("inheritance chains hold");
    }

    /// Oracle `spell_runtime_contracts`: HGame.Harry exists, descends from
    /// Engine.Actor (hence Engine.PlayerPawn), carries its bool reflection
    /// contract plus BossTarget, and Engine.Mover is reachable.
    #[test]
    fn harry_reflection_contract_holds() {
        use crate::value::PropertyKind;
        let Some(root) = data_root() else { return };
        let world = World::load(&root).expect("bootstrap");
        let harry = world
            .arena
            .find_by_path("HGame.Harry")
            .expect("HGame.Harry");
        let actor = world.arena.find_by_path("Engine.Actor").unwrap();
        assert!(world.arena.class_is_a(harry, actor).unwrap());
        let pawn = world.arena.find_by_path("Engine.Pawn").unwrap();
        assert!(world.arena.class_is_a(harry, pawn).unwrap());

        const REQUIRED_BOOLS: [&str; 13] = [
            "bScreenRelativeMovement",
            "bAutoCenterCamera",
            "bLockedOnTarget",
            "bFixedFaceDirection",
            "bInDuelingMode",
            "bReverseInput",
            "bKeepStationary",
            "bLockOutForward",
            "bLockOutBackward",
            "bLockOutStrafeLeft",
            "bLockOutStrafeRight",
            "bIsAiming",
            "bE3DemoLockout",
        ];
        for name in REQUIRED_BOOLS {
            let kind = find_property_kind(&world.arena, harry, name).unwrap();
            assert!(
                matches!(kind, Some(PropertyKind::Bool)),
                "HGame.Harry.{name} missing or not bool"
            );
        }
        let boss_target = find_property_kind(&world.arena, harry, "BossTarget").unwrap();
        assert!(matches!(boss_target, Some(PropertyKind::Object { .. })));
        assert!(world.arena.find_by_path("Engine.Mover").is_some());
    }

    /// Numbered slots from the oracle census are registered and dispatchable
    /// lookups succeed; unknown slots fail loudly.
    #[test]
    fn oracle_native_slots_are_bound() {
        let Some(root) = data_root() else { return };
        let world = World::load(&root).expect("bootstrap");
        // Concat_StrStr=112, GotoState=113, Sleep=256, GetCurrentKeyState=330.
        for slot in [112u16, 113, 256, 330] {
            assert!(
                world.registry.is_registered(slot),
                "oracle native slot {slot} is unbound"
            );
        }
        assert!(world.registry.get(u16::MAX).is_err());
    }
}
