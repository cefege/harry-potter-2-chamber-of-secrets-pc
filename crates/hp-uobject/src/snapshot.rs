//! Determinism harness: SHA-256 over the whole object space, ordered by
//! dotted id-path, with per-object sorted property names and raw payload
//! bytes. Two identical scripted runs must hash identically.

use sha2::{Digest, Sha256};

use crate::arena::{ObjectArena, ObjectData};

/// Hash the entire arena deterministically.
pub fn snapshot_world(arena: &ObjectArena) -> [u8; 32] {
    let mut hasher = Sha256::new();
    // Objects sorted by their dotted path (ties broken by id for duplicates).
    let mut entries: Vec<(String, u32)> = Vec::with_capacity(arena.len());
    for id in 0..arena.len() {
        let oid = crate::arena::ObjectId(id as u32);
        let path = arena.path_of(oid).unwrap_or_else(|_| format!("?{id}"));
        entries.push((path, id as u32));
    }
    entries.sort();

    for (path, id) in entries {
        let oid = crate::arena::ObjectId(id);
        let object = match arena.get(oid) {
            Ok(o) => o,
            Err(_) => continue,
        };
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(object.name_index.to_le_bytes());
        hasher.update(
            object
                .class_id
                .map(|c| c.0.to_le_bytes())
                .unwrap_or([0xFF; 4]),
        );
        hasher.update(object.outer.map(|o| o.0.to_le_bytes()).unwrap_or([0xFF; 4]));
        hasher.update(object.flags.to_le_bytes());
        hasher.update([object.data.discriminant_byte()]);
        match &object.data {
            ObjectData::Properties(store) => {
                for (name, value) in store.iter_sorted() {
                    hasher.update(name.to_le_bytes());
                    hasher.update(value.canonical_bytes());
                    hasher.update([0]);
                }
            }
            ObjectData::Raw(bytes) => {
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(bytes);
            }
            ObjectData::Function(function) => {
                hasher.update(function.native_index.to_le_bytes());
                hasher.update(function.function_flags.to_le_bytes());
                hasher.update((function.code.len() as u64).to_le_bytes());
                hasher.update(&function.code);
            }
            ObjectData::State(state) => {
                hasher.update(state.probe_mask.to_le_bytes());
                hasher.update(state.ignore_mask.to_le_bytes());
                hasher.update((state.code.len() as u64).to_le_bytes());
                hasher.update(&state.code);
            }
            ObjectData::Enum { names, .. } => {
                for name in names {
                    hasher.update(name.to_le_bytes());
                }
            }
            ObjectData::Const { value, .. } => {
                hasher.update(value.as_bytes());
            }
            ObjectData::TextBuffer { text } => {
                hasher.update(text.as_bytes());
            }
            _ => {}
        }
        hasher.update([0xFF]);
    }
    hasher.finalize().into()
}

impl ObjectData {
    /// Stable one-byte discriminant for hashing.
    fn discriminant_byte(&self) -> u8 {
        match self {
            ObjectData::Empty => 0,
            ObjectData::Properties(_) => 1,
            ObjectData::Class(_) => 2,
            ObjectData::Function(_) => 3,
            ObjectData::State(_) => 4,
            ObjectData::Property(_) => 5,
            ObjectData::ScriptStruct(_) => 6,
            ObjectData::Enum { .. } => 7,
            ObjectData::Const { .. } => 8,
            ObjectData::TextBuffer { .. } => 9,
            ObjectData::Raw(_) => 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::{ObjectArena, UObject};
    use crate::props::PropStore;
    use crate::value::PropValue;

    fn fresh_object(arena: &mut ObjectArena, name: &str) -> crate::arena::ObjectId {
        let name_index = arena.names.intern(name);
        arena.alloc(UObject {
            class_id: None,
            name_index,
            outer: None,
            flags: 0,
            data: ObjectData::Properties(PropStore::default()),
        })
    }

    /// Scripted N-tick sequence: each tick bumps two counters. Run twice from
    /// scratch and compare snapshots.
    fn run_ticks(ticks: usize) -> [u8; 32] {
        let mut arena = ObjectArena::new();
        let a = fresh_object(&mut arena, "TickA");
        let b = fresh_object(&mut arena, "TickB");
        for tick in 0..ticks {
            for (id, base) in [(a, 1u32), (b, 7)] {
                let object = arena.get_mut(id).unwrap();
                let store = object.properties_mut().unwrap();
                store.set(base, PropValue::Int(tick as i32));
                store.set(base + 100, PropValue::Bool(tick % 2 == 0));
            }
        }
        snapshot_world(&arena)
    }

    #[test]
    fn double_run_snapshots_are_equal() {
        let first = run_ticks(16);
        let second = run_ticks(16);
        assert_eq!(first, second, "identical tick sequences must hash equal");
    }

    #[test]
    fn divergent_runs_hash_differently() {
        let baseline = run_ticks(16);
        let divergent = run_ticks(17);
        assert_ne!(baseline, divergent);
    }

    /// Full-world determinism: two independent bootstraps of the stock
    /// packages must produce byte-identical SHA-256 world snapshots.
    #[test]
    fn real_world_double_run_hashes_equal() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            eprintln!("data-prototype root absent; skipping");
            return;
        }
        let first = crate::bootstrap::World::load(&root).expect("first bootstrap");
        let second = crate::bootstrap::World::load(&root).expect("second bootstrap");
        let a = snapshot_world(&first.arena);
        let b = snapshot_world(&second.arena);
        assert_eq!(a, b, "two identical loads must hash equal");
        println!(
            "snapshot_world(Core+Engine+UWindow+HPParticle+HGame) sha256 = {}",
            a.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
    }

    #[test]
    fn empty_arenas_hash_stably() {
        let a = snapshot_world(&ObjectArena::new());
        let b = snapshot_world(&ObjectArena::new());
        assert_eq!(a, b);
    }
}
