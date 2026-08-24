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
    ByteCursor, PROPERTY_TYPE_BOOL, PROPERTY_TYPE_STRUCT, PackageArchive, PropertyTag,
    read_compact_index, read_package,
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
    /// Arena ids for every export, indexed by export-table position.
    pub export_ids: Vec<ObjectId>,
    /// The parsed map package itself (`Polys` payloads live here); `None`
    /// only for the bootstrap placeholder level.
    pub archive: Option<PackageArchive>,
}

/// Resolve a harness map token (`..\Maps\PrivetDr.unr`, `Maps/Entry.unr`,
/// or a bare `Name.unr`). Primary anchor is `<data_root>/System` (engine
/// working directory); when that file does not exist, fall back to the
/// datadir root, where installed layouts keep `Maps/` next to `System/`
/// (`<datadir>/Maps/PrivetDr.unr`).
pub fn resolve_map_path(data_root: &Path, map_token: &str) -> PathBuf {
    let relative = map_token.replace('\\', "/");
    let candidate = PathBuf::from(&relative);
    if candidate.is_absolute() {
        return candidate;
    }
    let system_anchored = normalize_lexical(&data_root.join("System").join(&candidate));
    if system_anchored.is_file() {
        return system_anchored;
    }
    let datadir_anchored = normalize_lexical(&data_root.join(&candidate));
    if datadir_anchored.is_file() {
        return datadir_anchored;
    }
    system_anchored
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
            entry.class_ref,
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
        export_ids,
        archive: Some(archive),
    })
}

/// Read an actor's tagged-property list using the engine's bespoke
/// array-index encoding (`FPropertyTag::operator<<`: 1-, 2-, or 4-byte
/// form — NOT a compact index).
pub(crate) fn read_actor_tags(
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
        // HP2 packs StructProperty at raw type 10 in this wire (not the
        // imported PROPERTY_TYPE_STRUCT constant); both are accepted.
        let struct_name_index = if kind == PROPERTY_TYPE_STRUCT || kind == 10 {
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

/// Build a `PropValue::Struct`, interning field names on demand.
fn struct_value(
    arena: &mut ObjectArena,
    struct_name: &str,
    fields: &[(&str, PropValue)],
) -> PropValue {
    let (struct_index, _) = arena.names.intern_split(struct_name);
    PropValue::Struct {
        struct_name: struct_index,
        fields: fields
            .iter()
            .map(|(name, value)| {
                let (index, _) = arena.names.intern_split(name);
                (index, value.clone())
            })
            .collect(),
    }
}

/// Skip the HP2 per-object binary prologue when present.
///
/// Rule (validated on 6630/6630 prefixed exports across three maps):
/// present iff the first compact index decodes to the export's own
/// class reference and is immediately repeated; layout
/// `[ci class_ref][ci class_ref][FF*8][i32 V][u8 0x81]`, 14-16 bytes.
/// Semantics unidentified; treated as opaque.
pub(crate) fn skip_object_header(cur: &mut ByteCursor<'_>, class_ref: i32) {
    let Ok(first) = read_compact_index(cur) else {
        return;
    };
    if first != class_ref {
        cur.seek(0);
        return;
    }
    let save = cur.position();
    let Ok(second) = read_compact_index(cur) else {
        cur.seek(0);
        return;
    };
    if second != first {
        cur.seek(0);
        return;
    }
    // FF*8 + i32 V + u8 0x81
    if cur.len() < cur.position() + 8 + 4 + 1 {
        cur.seek(0);
        return;
    }
    let tail: Option<[u8; 8]> = cur.take(8).ok().and_then(|b| b.try_into().ok());
    let _ = cur.i32();
    let marker = cur.u8();
    match (tail, marker) {
        (Some(bytes), Ok(0x81)) if bytes == [0xff; 8] => {}
        _ => cur.seek(0),
    }
    let _ = save;
}

/// `bind.default_tag_raw` applies to fork defaults.
fn apply_actor_tags(
    arena: &mut ObjectArena,
    chain: &[ObjectId],
    payload: &[u8],
    class_ref: i32,
    name_map: &[u32],
    local_names: &[hp_format::package79::NameEntry],
    store: &mut PropStore,
) {
    let tags = {
        let mut cursor = ByteCursor::at(payload, 0);
        skip_object_header(&mut cursor, class_ref);
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
        // Placement structs whose Engine.u ScriptStruct children do not
        // flatten through the generic loader are decoded directly by their
        // declared struct name.
        let struct_text: Option<String> = tag
            .struct_name_index
            .and_then(|i| local_names.get(i as usize))
            .map(|n| n.text.to_ascii_lowercase());
        fn direct_struct_decode(
            arena: &mut ObjectArena,
            _kind: u8,
            struct_text: Option<&str>,
            payload: &[u8],
        ) -> Option<Result<(usize, PropValue)>> {
            use PROPERTY_TYPE_STRUCT;
            let _ = PROPERTY_TYPE_STRUCT;
            let mut c = ByteCursor::at(payload, 0);
            fn f32le(c: &mut ByteCursor<'_>) -> Result<f32> {
                let b: [u8; 4] = c
                    .take(4)
                    .map_err(EngineError::from)?
                    .try_into()
                    .map_err(|_| EngineError::new("engine.actor_tag_size", "short f32"))?;
                Ok(f32::from_le_bytes(b))
            }
            match struct_text {
                Some("vector") if payload.len() == 12 => {
                    let xyz = (f32le(&mut c), f32le(&mut c), f32le(&mut c));
                    let (x, y, z) = match (xyz.0, xyz.1, xyz.2) {
                        (Ok(x), Ok(y), Ok(z)) => (x, y, z),
                        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                            return Some(Err(e));
                        }
                    };
                    Some(Ok((
                        c.position(),
                        struct_value(
                            arena,
                            "Vector",
                            &[
                                ("X", PropValue::Float(x)),
                                ("Y", PropValue::Float(y)),
                                ("Z", PropValue::Float(z)),
                            ],
                        ),
                    )))
                }
                Some("rotator") => {
                    let pitch = c.i32().map_err(EngineError::from);
                    let yaw = c.i32().map_err(EngineError::from);
                    let roll = c.i32().map_err(EngineError::from);
                    let (pitch, yaw, roll) = match (pitch, yaw, roll) {
                        (Ok(p), Ok(y), Ok(r)) => (p, y, r),
                        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                            return Some(Err(e));
                        }
                    };
                    Some(Ok((
                        c.position(),
                        struct_value(
                            arena,
                            "Rotator",
                            &[
                                ("Pitch", PropValue::Int(pitch)),
                                ("Yaw", PropValue::Int(yaw)),
                                ("Roll", PropValue::Int(roll)),
                            ],
                        ),
                    )))
                }
                Some("scale") if payload.len() >= 17 => {
                    let sx = f32le(&mut c);
                    let sy = f32le(&mut c);
                    let sz = f32le(&mut c);
                    let sheer_rate = f32le(&mut c);
                    let sheer_axis = c.u8().map_err(EngineError::from);
                    let (sx, sy, sz, sheer_rate, sheer_axis) =
                        match (sx, sy, sz, sheer_rate, sheer_axis) {
                            (Ok(a), Ok(b), Ok(cc), Ok(d), Ok(e)) => (a, b, cc, d, e),
                            (Err(e), _, _, _, _)
                            | (_, Err(e), _, _, _)
                            | (_, _, Err(e), _, _)
                            | (_, _, _, Err(e), _)
                            | (_, _, _, _, Err(e)) => return Some(Err(e)),
                        };
                    Some(Ok((
                        c.position(),
                        struct_value(
                            arena,
                            "Scale",
                            &[
                                ("X", PropValue::Float(sx)),
                                ("Y", PropValue::Float(sy)),
                                ("Z", PropValue::Float(sz)),
                                ("SheerRate", PropValue::Float(sheer_rate)),
                                ("SheerAxis", PropValue::Byte(sheer_axis)),
                            ],
                        ),
                    )))
                }
                _ => None,
            }
        }
        let decoded = direct_struct_decode(arena, tag.kind, struct_text.as_deref(), &tag.payload)
            .unwrap_or_else(|| {
                decode_value(&mut payload_cur, &template.kind, &|struct_ref| {
                    hp_uobject::loader::flatten_struct_fields(arena, struct_ref)
                })
                .map(|v| (payload_cur.position(), v))
                .map_err(EngineError::from)
            });
        let value = match decoded {
            Ok((consumed, v)) if consumed == tag.payload.len() => v,
            Ok(_) | Err(_) => {
                eprintln!(
                    "hp-engine: note [engine.actor_tag_raw] tag `{text}` width disagrees with template ({} byte(s), consumed {}; kind={:?}; err={:?}); kept raw",
                    tag.payload.len(),
                    payload_cur.position(),
                    template.kind,
                    decoded.as_ref().err()
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
    fn falls_back_to_datadir_root_when_system_lacks_map() {
        let base = std::env::temp_dir().join(format!("hp2rs-map-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("System")).expect("System dir");
        std::fs::create_dir_all(base.join("Maps")).expect("Maps dir");
        std::fs::write(base.join("Maps/Entry.unr"), b"placeholder").expect("map file");
        assert_eq!(
            resolve_map_path(&base, "Maps\\Entry.unr"),
            base.join("Maps/Entry.unr"),
            "installed layouts keep Maps next to System"
        );
        // The System anchor still wins when both locations hold the file.
        std::fs::create_dir_all(base.join("System/Maps")).expect("System/Maps dir");
        std::fs::write(base.join("System/Maps/Entry.unr"), b"placeholder").expect("map file");
        assert_eq!(
            resolve_map_path(&base, "Maps\\Entry.unr"),
            normalize_lexical(&base.join("System/Maps/Entry.unr"))
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod diag {
    use super::*;
    #[test]
    fn diag_props() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("System/Core.u").is_file() {
            return;
        }
        let mut world = hp_uobject::bootstrap::World::load(&root).unwrap();
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).unwrap();
        let archive = hp_format::package79::read_package(&bytes).unwrap();
        for (index, entry) in archive.exports.iter().enumerate() {
            let name = archive
                .names
                .get(entry.object_name_index.max(0) as usize)
                .map(|n| n.text.as_str());
            let class_text = if entry.class_ref < 0 {
                archive
                    .imports
                    .get((-entry.class_ref - 1) as usize)
                    .and_then(|e| archive.names.get(e.object_name_index.max(0) as usize))
                    .map(|n| n.text.as_str())
            } else {
                None
            };
            if index < 3 {
                let payload = archive.export_payload(index).unwrap_or(&[]);
                println!(
                    "export[{index}] name={name:?} class={class_text:?} outer={} size={} bytes={:02x?}",
                    entry.outer_ref,
                    payload.len(),
                    payload
                );
            }
        }
        let level = load_level_from_bytes(&mut world.arena, "PrivetDr", &bytes).unwrap();
        let mut filled = 0;
        let mut empty = 0;
        for id in &level.actors {
            match world.arena.get(*id).unwrap().properties() {
                Some(store) if !store.is_empty() => filled += 1,
                _ => empty += 1,
            }
        }
        println!(
            "actors={} with-props={} empty-store={empty}",
            level.actors.len(),
            filled
        );
    }
}
