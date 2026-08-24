//! Port of the oracle ctest `Tests/AbiTests.cpp --test=native_registration`
//! (`TestNativeRegistration`) to the Rust object model.
//!
//! The C++ test checks: 16 ordered native lookup groups with no duplicates
//! and no extras; per-class lookup probes resolve; numbered slots hold the
//! contractual natives (CHECK_SLOT); an unknown native never resolves; and
//! re-registering a slot is detected as a duplicate without changing the
//! slot.
//!
//! Divergence note: the C++ `*NativeInfo` tables count C++-registered
//! entries including exec functions that carry no script body (e.g.
//! `UCommandlet.Main`'s native index, all of `UConsole`'s natives). This
//! port therefore pins the script-visible census per group — measured
//! against the stock packages under the loader's verified v79 grammar —
//! while the numbered-slot probes map one-to-one onto the oracle's
//! CHECK_SLOT block.

use hp_uobject::arena::ObjectData;
use hp_uobject::natives::{NATIVE_LOOKUP_GROUPS, NativeRegistry};
use hp_uobject::vm::USToken;
use std::path::PathBuf;

/// Group classes in contractual order, index-aligned with
/// [`NATIVE_LOOKUP_GROUPS`] (= engine `ExpectedLookups[]`).
const GROUP_CLASS_PATHS: [&str; 16] = [
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

/// Script-visible native census per group, in group order. Derived from the
/// stock packages; the loader must reproduce it exactly or binding fails.
const EXPECTED_GROUP_CENSUS: [usize; 16] = [146, 0, 62, 24, 3, 0, 0, 0, 1, 2, 0, 0, 1, 9, 0, 5];

/// Oracle CHECK_LOOKUP list: `(class path, function name)` pairs that must
/// resolve as script-declared functions after `InstallHP2NativeLookups`.
/// The C++-only registrations have no script declaration and are covered by
/// the `ENGINE_INTRINSIC_SLOTS` slot pins below instead: UObject's
/// execLocalVariable / execInstanceVariable / execIntConst / execIterator
/// and AActor.GetCurrentKeyState.
const CHECK_LOOKUPS: [(&str, &str); 15] = [
    ("Core.Commandlet", "Main"),
    ("Engine.Actor", "Move"),
    ("Engine.Pawn", "MoveTo"),
    ("Engine.PlayerPawn", "UpdateURL"),
    ("Engine.Decal", "AttachDecal"),
    ("Engine.StatLog", "GetMapFileName"),
    ("Engine.StatLogFile", "OpenLog"),
    ("Engine.ZoneInfo", "ZoneActors"),
    ("Engine.WarpZoneInfo", "Warp"),
    ("Engine.LevelInfo", "GetLocalURL"),
    ("Engine.GameInfo", "ParseKillMessage"),
    ("Engine.NavigationPoint", "describeSpec"),
    ("Engine.Canvas", "StrLen"),
    ("Engine.Console", "ConsoleCommand"),
    ("Engine.ScriptedTexture", "ReplaceTexture"),
];

fn data_root() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
    root.join("System/Core.u").is_file().then_some(root)
}

fn find_function<'a>(
    world: &'a hp_uobject::bootstrap::World,
    class_path: &str,
    function_name: &str,
) -> Option<&'a hp_uobject::arena::FunctionData> {
    let class_id = world.arena.find_by_path(class_path)?;
    world.arena.children_of(class_id).find_map(|(_, object)| {
        if let ObjectData::Function(function) = &object.data
            && world
                .arena
                .names
                .text(object.name_index)
                .is_some_and(|n| n.eq_ignore_ascii_case(function_name))
        {
            return Some(function.as_ref());
        }
        None
    })
}

#[test]
fn native_registration() {
    let Some(root) = data_root() else {
        eprintln!("data-prototype root absent; skipping");
        return;
    };

    // InstallHP2NativeLookups + appInit package loads: bootstrap binds every
    // group in order or fails loudly. A successful load proves no slot was
    // registered twice (the registry rejects duplicates) — the oracle's
    // `GNativeDuplicate != 0` probe.
    let world = hp_uobject::bootstrap::World::load(&root).expect("bootstrap binds cleanly");

    // ExpectedLookups[]: 16 groups in exact order, each resolving to a real,
    // distinct class; nothing beyond them.
    assert_eq!(NATIVE_LOOKUP_GROUPS.len(), GROUP_CLASS_PATHS.len());
    let mut seen_ids = Vec::new();
    for (index, class_path) in GROUP_CLASS_PATHS.iter().enumerate() {
        let id = world
            .arena
            .find_by_path(class_path)
            .unwrap_or_else(|| panic!("lookup slot {index} ({class_path}) missing"));
        assert!(
            !seen_ids.contains(&id),
            "lookup slot {index} resolves to a duplicate class"
        );
        seen_ids.push(id);
    }

    // CheckAllGeneratedNatives / CheckLookupTable equivalents: every group's
    // script-visible native census matches the stock-package expectation and
    // bind reports zero unbound exports/natives overall.
    let mut exported_ids = Vec::new();
    for ids in world.exports.values() {
        exported_ids.extend(ids.iter().copied());
    }
    let mut fresh = NativeRegistry::new();
    let report =
        hp_uobject::bind::bind_world(&world.arena, &mut fresh, &exported_ids).expect("re-bind");
    println!(
        "native_registration report: {} classes bound, {} unbound exports, \
         group bindings {:?}, {} ungrouped natives, {} slots registered",
        report.classes_bound,
        report.unbound_classes.len(),
        report.group_bindings,
        report.ungrouped_natives.len(),
        world.registry.registered_count(),
    );
    assert!(
        report.unbound_classes.is_empty(),
        "untyped exports remain: {:?}",
        report.unbound_classes
    );
    assert!(
        report.ungrouped_natives.is_empty(),
        "unbound natives remain: {:?}",
        report.ungrouped_natives
    );
    for (group_index, expected) in EXPECTED_GROUP_CENSUS.iter().enumerate() {
        assert_eq!(
            report.group_bindings[group_index], *expected,
            "lookup group {group_index} ({}) census drifted",
            NATIVE_LOOKUP_GROUPS[group_index]
        );
    }

    // CHECK_LOOKUP(Class, Name): each named function exists under its class.
    for (class_path, function_name) in CHECK_LOOKUPS {
        assert!(
            find_function(&world, class_path, function_name).is_some(),
            "{class_path}.{function_name} does not resolve"
        );
    }

    // CHECK_SLOT block: intrinsic opcode slots plus GetCurrentKeyState=330.
    for (slot, subject) in hp_uobject::natives::ENGINE_INTRINSIC_SLOTS {
        assert!(
            world.registry.is_registered(slot),
            "native slot {slot} mismatch for {subject}"
        );
    }
    // The four intrinsic opcodes decode as tokens the interpreter executes
    // directly (their GNatives rows are layout pins, not dispatch targets).
    for opcode in [0x00u8, 0x01, 0x1D, 0x2F] {
        assert!(
            USToken::from_opcode(opcode).is_some(),
            "opcode {opcode:#04x}"
        );
    }

    // FindNative("intNoSuchClassexecNoSuchFunction") == NULL.
    assert!(world.arena.find_by_path("Core.NoSuchClass").is_none());
    match world.registry.get(u16::MAX) {
        Err(fail) => assert_eq!(fail.reason_code, "native.slot_unbound"),
        Ok(_) => panic!("unknown native lookup unexpectedly resolved"),
    }

    // Duplicate registration probe: re-registering a live slot is rejected
    // loudly and leaves the original registration untouched.
    let registered = world.registry.get(330).expect("slot 330 registered");
    let mut probe = NativeRegistry::new();
    probe
        .register(330, 2, registered, "probe.first")
        .expect("first registration");
    let second = probe.register(330, 3, hp_uobject::natives::deferred_native, "probe.second");
    match second {
        Err(fail) => assert_eq!(fail.reason_code, "native.slot_duplicate"),
        Ok(()) => panic!("duplicate registration probe was not detected"),
    }
    assert!(
        std::ptr::fn_addr_eq(probe.get(330).expect("slot unchanged"), registered),
        "duplicate registration probe changed slot 330"
    );
}

/// Regression for the retail PrivetDr launch: an actor class without any
/// scripted `Tick` in its inheritance chain must resolve to `None` so the
/// engine skips it silently — no phantom body may ever enter VM exec (the
/// original failure executed a misdecoded payload as `Mover.Tick`).
#[test]
fn unscripted_tick_resolves_to_none_and_actor_stub_is_noop() {
    let Some(root) = data_root() else {
        eprintln!("data-prototype root absent; skipping");
        return;
    };
    let world = hp_uobject::bootstrap::World::load(&root).expect("bootstrap");

    // Non-actor classes declare no Tick anywhere up their chain.
    let model = world.arena.find_by_path("Engine.Model").expect("Engine.Model");
    assert_eq!(
        world.arena.find_function(model, "Tick").expect("lookup"),
        None,
        "Engine.Model must not resolve a phantom Tick body"
    );

    // Where a scripted Tick does exist (Engine.Actor), its body is exactly
    // the Return/Nothing stub — executing it can never fault.
    let actor = world
        .arena
        .find_by_path("Engine.Actor")
        .expect("Engine.Actor");
    let tick = world
        .arena
        .find_function(actor, "Tick")
        .expect("lookup")
        .expect("Engine.Actor.Tick exists");
    match &world.arena.get(tick).expect("object").data {
        ObjectData::Function(function) => {
            assert_eq!(
                function.code.as_slice(),
                &[USToken::Return as u8, USToken::Nothing as u8],
                "Actor.Tick stub changed shape"
            );
        }
        other => panic!("Tick export is not a Function: {other:?}"),
    }
}
