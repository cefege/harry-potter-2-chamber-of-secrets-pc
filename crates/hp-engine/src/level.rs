//! M1 — level bootstrap: load a `.unr` map package, instantiate its actor
//! exports into the shared arena in export order, and expose the actor
//! list plus BSP-model census the tick loop needs.
//!
//! Actor properties decode against the Engine/HGame class templates
//! already bound by `hp_uobject::bootstrap`; payloads keep raw package
//! references exactly like every other loaded instance.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hp_format::package79::{
    ByteCursor, PROPERTY_TYPE_BOOL, PROPERTY_TYPE_STRUCT, PropertyTag, read_compact_index,
    read_package,
};
use hp_uobject::arena::{ObjectArena, ObjectData, ObjectId};
use hp_uobject::props::PropStore;
use hp_uobject::value::{PropValue, PropertyKind, decode_value};

use crate::error::{EngineError, Result};

/// One loaded map.
#[derive(Debug)]
pub struct Level {
    pub root: ObjectId,
    /// Every instantiated actor, in export order.
    pub actors: Vec<ObjectId>,
    /// BSP model exports referenced by the level surface graph.
    pub model_count: usize,
}

/// Resolve a harness map token (`..\Maps\PrivetDr.unr`, `Maps/Entry.unr`,
/// or a bare `Name.unr`) to the map file under `<data_root>/System`.
pub fn resolve_map_path(data_root: &Path, map_token: &str) -> PathBuf {
    let relative = map_token.replace('\\', "/");
    let candidate = PathBuf::from(&relative);
    if candidate.is_absolute() {
        return candidate;
    }
    normalize_lexical(&data_root.join("System").join(candidate))
}

/// Collapse `.` and `..` components lexically.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Load one map into the arena. Every non-script export instantiates as an
/// actor; anything the script packages would own is a loud gap.
pub fn load_level(arena: &mut ObjectArena, data_root: &Path, map_token: &str) -> Result<Level> {
    let map_path = resolve_map_path(data_root, map_token);
    let source = std::fs::read(&map_path).map_err(|error| {
        EngineError::new(
            "engine.map_unreadable",
            format!("{}: {error}", map_path.display()),
        )
    })?;
    let stem = map_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Map")
        .to_string();
    load_level_from_bytes(arena, &stem, &source)
}

/// Load one map from already-read bytes (also the test seam).
pub fn load_level_from_bytes(
    arena: &mut ObjectArena,
    map_name: &str,
    source: &[u8],
) -> Result<Level> {
    let archive = read_package(source)
        .map_err(|error| EngineError::new("engine.map_parse", error.to_string()))?;

    // Merge this package's names into the shared pool.
    let mut name_map = Vec::with_capacity(archive.names.len());
    for entry in &archive.names {
        name_map.push(arena.names.intern(&entry.text));
    }

    let root = arena.root_for(map_name);

    // Imports: stitch to objects that already exist (Engine classes), else
    // allocate skeletons so references have somewhere to point.
    let mut import_ids: Vec<Option<ObjectId>> = vec![None; archive.imports.len()];
    for i in 0..archive.imports.len() {
        let entry = archive.imports[i];
        let name_index = *name_map
            .get(entry.object_name_index.max(0) as usize)
            .ok_or_else(|| EngineError::new("engine.map_name_index", "import name"))?;
        let parent: Option<ObjectId> = if entry.outer_ref == 0 {
            None
        } else {
            import_ids[(-entry.outer_ref - 1) as usize]
        };
        let mut path = String::new();
        if let Some(parent) = parent {
            path.push_str(&arena.path_of(parent)?);
            path.push('.');
        }
        path.push_str(arena.names.text(name_index).unwrap_or("?"));
        import_ids[i] = Some(match arena.find_by_path(&path) {
            Some(existing) => existing,
            None => arena.alloc(hp_uobject::arena::UObject {
                class_id: None,
                name_index,
                outer: parent,
                flags: 0,
                data: ObjectData::Empty,
            }),
        });
    }

    // Export skeletons first so forward outer references resolve.
    let mut export_ids: Vec<ObjectId> = Vec::with_capacity(archive.exports.len());
    for _ in &archive.exports {
        export_ids.push(arena.alloc(hp_uobject::arena::UObject::default()));
    }
    for (index, entry) in archive.exports.iter().enumerate() {
        let name_index = *name_map
            .get(entry.object_name_index.max(0) as usize)
            .ok_or_else(|| EngineError::new("engine.map_name_index", "export name"))?;
        let outer = if entry.outer_ref == 0 {
            Some(root)
        } else if entry.outer_ref > 0 {
            export_ids.get(entry.outer_ref as usize - 1).copied()
        } else {
            import_ids[(-entry.outer_ref - 1) as usize]
        };
        let object = arena.get_mut(export_ids[index])?;
        object.class_id = None;
        object.name_index = name_index;
        object.outer = outer;
        object.flags = entry.object_flags;
        object.data = ObjectData::Empty;
    }

    // Reconcile import skeletons that this map itself exports.
    let first_export = export_ids.first().map(|id| id.0).unwrap_or(u32::MAX);
    for slot in import_ids.iter_mut() {
        if let Some(sid) = *slot
            && sid.0 < first_export
        {
            let path = arena.path_of(sid)?;
            if let Some(real) = arena.find_by_path(&path)
                && real.0 >= first_export
            {
                *slot = Some(real);
            }
        }
    }

    // Resolve a package reference to its arena id.
    let resolve = |reference: i32| -> Option<ObjectId> {
        if reference == 0 {
            None
        } else if reference > 0 {
            export_ids.get(reference as usize - 1).copied()
        } else {
            import_ids.get((-reference - 1) as usize).copied().flatten()
        }
    };

    // Instantiate every actor in export order.
    let mut actors = Vec::with_capacity(export_ids.len());
    let mut model_count = 0usize;
    let mut gaps: Vec<(ObjectId, String)> = Vec::new();
    for (index, entry) in archive.exports.iter().enumerate() {
        let id = export_ids[index];
        let Some(class_id) = resolve(entry.class_ref) else {
            gaps.push((id, "export carries no resolvable class reference".into()));
            continue;
        };
        let folded = arena
            .get(class_id)
            .ok()
            .and_then(|object| {
                arena
                    .names
                    .text(object.name_index)
                    .map(str::to_ascii_lowercase)
            })
            .unwrap_or_default();

        if matches!(folded.as_str(), "class" | "function" | "state") {
            gaps.push((
                id,
                format!("script metadata export ({folded}) cannot appear in a map"),
            ));
            continue;
        }
        if folded == "model" {
            model_count += 1;
        }

        let payload = archive.export_payload(index).unwrap_or(&[]);
        let chain = arena.class_chain(class_id)?;
        let mut store = PropStore::new();
        apply_actor_tags(
            arena,
            &chain,
            payload,
            &name_map,
            &archive.names,
            &mut store,
        );
        let object = arena.get_mut(id)?;
        object.class_id = Some(class_id);
        object.data = ObjectData::Properties(store);
        actors.push(id);
    }

    if !gaps.is_empty() {
        for (id, message) in &gaps {
            let subject = arena.path_of(*id).unwrap_or_else(|_| format!("{id:?}"));
            eprintln!("hp-engine: [engine.map_export_gap] at {subject}: {message}");
        }
        return Err(EngineError::new(
            "engine.map_export_gap",
            format!(
                "{map_name}: {} of {} exports failed to instantiate",
                gaps.len(),
                archive.exports.len()
            ),
        ));
    }

    Ok(Level {
        root,
        actors,
        model_count,
    })
}

/// Read an actor's tagged-property list using the engine's bespoke
/// array-index encoding (`FPropertyTag::operator<<`: 1-, 2-, or 4-byte
/// form — NOT a compact index).
fn read_actor_tags(
    cur: &mut ByteCursor<'_>,
    names: &[hp_format::package79::NameEntry],
) -> Result<Vec<PropertyTag>> {
    let mut tags = Vec::new();
    loop {
        let name_index = read_compact_index(cur)?;
        if name_index < 0 || name_index as usize >= names.len() {
            return Err(EngineError::new(
                "engine.actor_tag_range",
                format!("tag name {name_index} / {}", names.len()),
            ));
        }
        if names[name_index as usize].text == "None" {
            return Ok(tags);
        }
        let info = cur.u8()?;
        let kind = info & 0x0F;
        let size_code = info & 0x70;
        let array_flag = info & 0x80 != 0;
        let struct_name_index = if kind == PROPERTY_TYPE_STRUCT {
            Some(read_compact_index(cur)?)
        } else {
            None
        };
        let size: usize = match size_code {
            0x00 => 1,
            0x10 => 2,
            0x20 => 4,
            0x30 => 12,
            0x40 => 16,
            0x50 => usize::from(cur.u8()?),
            0x60 => usize::from(cur.u16()?),
            _ => {
                let v = cur.i32()?;
                usize::try_from(v).map_err(|_| {
                    EngineError::new("engine.actor_tag_size", format!("negative size {v}"))
                })?
            }
        };
        let array_index = if array_flag && kind != PROPERTY_TYPE_BOOL {
            let b0 = cur.u8()?;
            Some(if b0 & 0x80 == 0 {
                i32::from(b0)
            } else if b0 & 0xC0 == 0x80 {
                let c = cur.u8()?;
                ((i32::from(b0 & 0x7F)) << 8) | i32::from(c)
            } else {
                let c = cur.u8()?;
                let d = cur.u8()?;
                let e = cur.u8()?;
                ((i32::from(b0 & 0x3F)) << 24)
                    | ((i32::from(c)) << 16)
                    | ((i32::from(d)) << 8)
                    | i32::from(e)
            })
        } else {
            None
        };
        let payload = cur.take(size)?.to_vec();
        tags.push(PropertyTag {
            name_index,
            kind,
            size_code,
            array_flag,
            struct_name_index,
            array_index,
            payload,
        });
    }
}

/// Decode one actor payload's tags into `store` against its class chain.
///
/// A tag whose template is missing or whose decoded width disagrees keeps
/// a verbatim note on stderr (reason `engine.actor_tag_raw`) instead of
/// failing the whole map — the same retention rule
/// `bind.default_tag_raw` applies to fork defaults.
fn apply_actor_tags(
    arena: &ObjectArena,
    chain: &[ObjectId],
    payload: &[u8],
    name_map: &[u32],
    local_names: &[hp_format::package79::NameEntry],
    store: &mut PropStore,
) {
    let tags = {
        let mut cursor = ByteCursor::at(payload, 0);
        match read_actor_tags(&mut cursor, local_names) {
            Ok(tags) => tags,
            Err(error) => {
                eprintln!(
                    "hp-engine: note [engine.actor_tag_raw] {error}; actor kept with no serialized properties"
                );
                return;
            }
        }
    };
    let mut fixed_arrays: HashMap<u32, std::collections::BTreeMap<i32, PropValue>> = HashMap::new();

    for tag in tags {
        let text = local_names[tag.name_index as usize].text.as_str();
        let name_global = name_map[tag.name_index as usize];
        if tag.kind == PROPERTY_TYPE_BOOL {
            store.set(name_global, PropValue::Bool(tag.array_flag));
            continue;
        }
        let template = hp_uobject::loader::find_template(arena, chain[0], name_global)
            .ok()
            .flatten();
        let Some(template) = template else {
            eprintln!(
                "hp-engine: note [engine.actor_tag_raw] tag `{text}` has no template on the class chain; kept raw"
            );
            continue;
        };
        let mut payload_cur = ByteCursor::at(&tag.payload, 0);
        let decoded = decode_value(&mut payload_cur, &template.kind, &|struct_ref| {
            hp_uobject::loader::flatten_struct_fields(arena, struct_ref)
        });
        let value = match decoded {
            Ok(v) if payload_cur.position() == tag.payload.len() => v,
            Ok(_) | Err(_) => {
                eprintln!(
                    "hp-engine: note [engine.actor_tag_raw] tag `{text}` width disagrees with template ({} byte(s)); kept raw",
                    tag.payload.len()
                );
                continue;
            }
        };
        match (&template.kind, tag.array_index) {
            (PropertyKind::FixedArray { .. }, Some(idx)) => {
                fixed_arrays
                    .entry(name_global)
                    .or_default()
                    .insert(idx, value);
            }
            (PropertyKind::FixedArray { .. }, None) => match value {
                PropValue::FixedArray(items) => {
                    store.set(name_global, PropValue::FixedArray(items))
                }
                other => store.set(name_global, other),
            },
            _ => store.set(name_global, value),
        }
    }

    for (name, parts) in fixed_arrays {
        let count = parts.keys().max().copied().unwrap_or(-1) as usize + 1;
        let items: Vec<PropValue> = (0..count)
            .map(|i| {
                parts
                    .get(&(i as i32))
                    .cloned()
                    .unwrap_or(PropValue::Byte(0))
            })
            .collect();
        store.set(name, PropValue::FixedArray(items));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_harness_tokens() {
        let root = Path::new("/data");
        assert_eq!(
            resolve_map_path(root, "..\\Maps\\PrivetDr.unr"),
            PathBuf::from("/data/Maps/PrivetDr.unr")
        );
        assert_eq!(
            resolve_map_path(root, "PrivetDr.unr"),
            PathBuf::from("/data/System/PrivetDr.unr")
        );
        assert_eq!(
            resolve_map_path(root, "Maps/Entry.unr"),
            PathBuf::from("/data/System/Maps/Entry.unr")
        );
    }

    #[test]
    fn rejects_truncated_maps() {
        let mut arena = ObjectArena::new();
        let error = load_level_from_bytes(&mut arena, "X", b"\x9e*\x83\xc1").unwrap_err();
        assert_eq!(error.reason_code, "engine.map_parse");
    }
}
