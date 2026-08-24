//! Native dispatch: the numbered-slot registry (`GNatives[0x1000]`) plus the
//! 16 ordered name groups mirroring `InstallHP2NativeLookups` in
//! `HarryPotter2/Unreal/Launch/Src/HP2StaticPackages.cpp`.
//!
//! Registration is data-driven: a group binds the native functions of one
//! class from the loaded arena into numbered slots. A missing slot entry at
//! dispatch time is a loud, reason-coded error — never a silent fallback.

use crate::arena::{ObjectArena, ObjectData, ObjectId};
use crate::error::{Fail, Result};

/// Highest legal `iNative` slot (packages reject anything above this).
pub const MAX_NATIVE_INDEX: u32 = 0x1000;

/// Engine-side natives: C++-registered slots that never appear as package
/// functions (the generated-registrant equivalents). Oracle:
/// `CHECK_SLOT(330, AActor, execGetCurrentKeyState)` plus the four
/// `UObject` VM intrinsics pinned by AbiTests' CHECK_SLOT block at their
/// `EExprToken` opcode slots. The interpreter executes the intrinsic
/// opcodes directly (`crate::vm`), so these rows pin the contractual slot
/// layout; dispatching into one stays a loud deferral.
pub const ENGINE_INTRINSIC_SLOTS: [(u16, &str); 5] = [
    (0x00, "Core.UObject.LocalVariable"),
    (0x01, "Core.UObject.InstanceVariable"),
    (0x1D, "Core.UObject.IntConst"),
    (0x2F, "Core.UObject.Iterator"),
    (330, "Engine.AActor.GetCurrentKeyState"),
];

/// One registered native body.
pub type NativeImpl = fn(&mut crate::vm::Frame<'_>) -> Result<crate::value::PropValue>;

/// The 16 ordered lookup groups of `InstallHP2NativeLookups` (2 Core + 14
/// Engine classes). Order is contractual; the oracle test fails on any
/// reordering.
pub const NATIVE_LOOKUP_GROUPS: [&str; 16] = [
    "Core.UObject",
    "Core.UCommandlet",
    "Engine.AActor",
    "Engine.APawn",
    "Engine.APlayerPawn",
    "Engine.ADecal",
    "Engine.AStatLog",
    "Engine.AStatLogFile",
    "Engine.AZoneInfo",
    "Engine.AWarpZoneInfo",
    "Engine.ALevelInfo",
    "Engine.AGameInfo",
    "Engine.ANavigationPoint",
    "Engine.UCanvas",
    "Engine.UConsole",
    "Engine.UScriptedTexture",
];

/// The numbered-slot table. `None` means "slot not registered".
pub struct NativeRegistry {
    slots: [Option<NativeImpl>; MAX_NATIVE_INDEX as usize],
    /// Slot → owning group index (diagnostics for unbound-slot reports).
    owners: [Option<u8>; MAX_NATIVE_INDEX as usize],
}

impl Default for NativeRegistry {
    fn default() -> Self {
        Self {
            slots: [const { None }; 4096],
            owners: [const { None }; 4096],
        }
    }
}

impl NativeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a body at `slot`, rejecting duplicates loudly.
    pub fn register(
        &mut self,
        slot: u16,
        group_index: usize,
        body: NativeImpl,
        subject: &str,
    ) -> Result<()> {
        let index = slot as usize;
        if index >= self.slots.len() {
            return Err(Fail::new(
                "native.slot_out_of_range",
                format!("{subject}: slot {slot} exceeds {MAX_NATIVE_INDEX:#x}"),
            ));
        }
        if let Some(previous) = self.owners[index] {
            return Err(Fail::new(
                "native.slot_duplicate",
                format!("{subject}: slot {slot} already owned by group {}", previous),
            ));
        }
        self.slots[index] = Some(body);
        self.owners[index] = Some(group_index as u8);
        Ok(())
    }

    /// Dispatch lookup; a missing entry is a loud error.
    pub fn get(&self, slot: u16) -> Result<NativeImpl> {
        self.slots
            .get(slot as usize)
            .copied()
            .flatten()
            .ok_or_else(|| {
                Fail::new(
                    "native.slot_unbound",
                    format!("native slot {slot} has no registered body"),
                )
            })
    }

    pub fn is_registered(&self, slot: u16) -> bool {
        self.slots.get(slot as usize).copied().flatten().is_some()
    }

    pub fn registered_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

/// Bind every `FUNC_Native` function of `class_id`'s chain-reachable children
/// into `registry` under `group_index`. Returns the bound count. Functions
/// whose bodies are deferred still occupy their slot via [`deferred_native`],
/// so dispatch fails loudly with the deferral reason instead of "unbound".
pub fn bind_class_natives(
    arena: &ObjectArena,
    registry: &mut NativeRegistry,
    group_index: usize,
    class_path: &str,
    class_id: ObjectId,
) -> Result<usize> {
    if arena.find_by_path(class_path) != Some(class_id) {
        return Err(Fail::new(
            "native.group_class_missing",
            format!("group {group_index}: {class_path} does not resolve to {class_id:?}"),
        ));
    }
    let mut bound = 0;
    for child in children_deep(arena, class_id)? {
        let ObjectData::Function(function) = &arena.get(child)?.data else {
            continue;
        };
        let function = (**function).clone();
        if function.native_index == 0 {
            continue;
        }
        let subject = arena.path_of(child)?;
        registry.register(
            function.native_index,
            group_index,
            deferred_native,
            &subject,
        )?;
        bound += 1;
    }
    Ok(bound)
}

fn children_deep(arena: &ObjectArena, class_id: ObjectId) -> Result<Vec<ObjectId>> {
    let mut out = Vec::new();
    for (child_id, object) in arena.children_of(class_id) {
        let _ = &object;
        out.push(child_id);
        if matches!(arena.get(child_id)?.data, ObjectData::Class(_)) {
            out.extend(children_deep(arena, child_id)?);
        }
    }
    Ok(out)
}

/// Placeholder body for slots whose implementation ships with the engine
/// runtime phase; dispatching into it fails loudly per reason-code policy.
pub fn deferred_native(_frame: &mut crate::vm::Frame<'_>) -> Result<crate::value::PropValue> {
    Err(Fail::new(
        "native.body_deferred",
        "native body is registered but its implementation is deferred to the engine-runtime phase",
    ))
}

/// Collect `(native_index, path)` for every native function under `class_id`.
pub fn native_census(arena: &ObjectArena, class_id: ObjectId) -> Result<Vec<(u16, String)>> {
    let mut out = Vec::new();
    for (child_id, _) in arena.children_of(class_id) {
        if let Ok(ObjectData::Function(function)) = arena.get(child_id).map(|o| &o.data)
            && function.native_index != 0
        {
            out.push((function.native_index, arena.path_of(child_id)?));
        }
    }
    Ok(out)
}
