//! M1 — scene extraction: build a [`RenderScene`] from a loaded map.
//!
//! HP2 maps ship editor-template geometry: every `Engine.Model` export owns
//! a `Polys` subobject (a `UPolys` array of `FPoly`s, wire format per
//! `UModel.cpp::operator<<(FArchive&, FPoly&)`) and no prebuilt BSP
//! (`Nodes`/`Surfs`/`Verts` exports are absent from retail maps). World
//! geometry therefore comes from brush polygons transformed by their owning
//! brush actor's `Location`/`Rotation` — the same facts the C++ dynamic-BSP
//! builder consumes.

use hp_format::package79::{
    ByteCursor, PackageArchive, PackageError, PackageResult, read_compact_index, read_property_tags,
};
use hp_uobject::arena::{ObjectArena, ObjectId};
use hp_uobject::props::PropStore;
use hp_uobject::value::PropValue;

use crate::error::{EngineError, Result};
use crate::level::Level;

/// Identity of one texture: the .utx package stem plus the dotted object
/// path of the `UTexture` inside it (e.g. `"PrivetDrive"`, `"walls.wall01"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureKey {
    pub package: String,
    /// Dot-separated object names from the package root down to the texture.
    pub object_path: String,
}

/// One world-space polygon with its texture mapping basis (wire facts from
/// `FPoly`, already moved into world space by the owning brush transform).
#[derive(Debug, Clone)]
pub struct ScenePoly {
    /// Texture-mapping origin (`FPoly::Base`) in world space.
    pub base: [f32; 3],
    pub vertices: Vec<[f32; 3]>,
    pub normal: [f32; 3],
    /// Texture U axis (world-space direction; length = world units per texel).
    pub tex_u: [f32; 3],
    /// Texture V axis (world-space direction).
    pub tex_v: [f32; 3],
    /// Texture panning in texels `(PanU, PanV)`.
    pub pan_uv: [f32; 2],
    /// `EPolyFlags` verbatim (`PF_Masked`, `PF_Translucent`, ...).
    pub poly_flags: u32,
    pub texture: Option<TextureKey>,
}

/// Camera seed resolved from a map actor (`PlayerStart`, else first `Camera`).
#[derive(Debug, Clone, Copy)]
pub struct SceneCamera {
    pub location: [f32; 3],
    /// `(Pitch, Yaw, Roll)` rotator units — 65536 = one revolution.
    pub rotation: [i32; 3],
}

impl Default for SceneCamera {
    fn default() -> Self {
        SceneCamera {
            location: [0.0, 0.0, 0.0],
            rotation: [0, 0, 0],
        }
    }
}

/// Everything the renderer needs for one map.
#[derive(Debug, Clone, Default)]
pub struct RenderScene {
    pub polys: Vec<ScenePoly>,
    pub camera: Option<SceneCamera>,
}

impl RenderScene {
    pub fn camera_or_default(&self) -> SceneCamera {
        self.camera.unwrap_or_default()
    }
}

/// One `FPoly` straight off the wire — world-space movement still pending.
#[derive(Debug, Clone)]
struct RawPoly {
    base: [f32; 3],
    normal: [f32; 3],
    tex_u: [f32; 3],
    tex_v: [f32; 3],
    vertices: Vec<[f32; 3]>,
    poly_flags: u32,
    texture_ref: i32,
    pan_uv: [f32; 2],
}

/// Brush placement read off its actor (`Location` plus UE1 rotator).
type BrushTransform = ([f32; 3], [i32; 3]);

/// Build the renderable scene for one loaded map.
///
/// Every `Model` export's `Polys` child carries the brush polygons; each
/// polygon is moved into world space by its owning brush actor transform and
/// its `Texture` reference resolved to a package-qualified [`TextureKey`].
/// All polys are kept — visibility classification happens downstream.
pub fn build_render_scene(arena: &ObjectArena, level: &Level) -> Result<RenderScene> {
    let Some(archive) = level.archive.as_ref() else {
        return Err(EngineError::new(
            "engine.scene_no_map",
            "level has no map archive (bootstrap placeholder)",
        ));
    };

    let mut polys = Vec::new();

    for (index, entry) in archive.exports.iter().enumerate() {
        let Some(class_text) = ref_class_text(archive, entry.class_ref) else {
            continue;
        };
        if !class_text.eq_ignore_ascii_case("Model") {
            continue;
        }
        let (location, rotation) = brush_transform(arena, level, entry.outer_ref);
        let Some(payload) = archive.export_payload(index) else {
            continue;
        };
        let polys_ref = model_polys_ref(payload, archive)?;
        if polys_ref <= 0 {
            continue;
        }
        let Some(polys_payload) = archive.export_payload(polys_ref as usize - 1) else {
            continue;
        };
        for raw in parse_upolys(polys_payload, archive)? {
            polys.push(ScenePoly {
                base: translate(apply_rotator(rotation, raw.base), location),
                vertices: raw
                    .vertices
                    .iter()
                    .map(|v| translate(apply_rotator(rotation, *v), location))
                    .collect(),
                normal: apply_rotator(rotation, raw.normal),
                tex_u: apply_rotator(rotation, raw.tex_u),
                tex_v: apply_rotator(rotation, raw.tex_v),
                pan_uv: raw.pan_uv,
                poly_flags: raw.poly_flags,
                texture: resolve_texture_key(archive, raw.texture_ref),
            });
        }
    }

    Ok(RenderScene {
        polys,
        camera: find_camera(arena, level),
    })
}

/// Name text for a local name-table index.
fn archive_name_text(archive: &PackageArchive, index: i32) -> Option<&str> {
    archive
        .names
        .get(index.max(0) as usize)
        .map(|n| n.text.as_str())
}

/// Object-name text behind a compact signed reference (import or export).
fn ref_class_text(archive: &PackageArchive, class_ref: i32) -> Option<&str> {
    if class_ref > 0 {
        let entry = archive.exports.get(class_ref as usize - 1)?;
        archive_name_text(archive, entry.object_name_index)
    } else if class_ref < 0 {
        let entry = archive.imports.get((-class_ref - 1) as usize)?;
        archive_name_text(archive, entry.object_name_index)
    } else {
        None
    }
}

/// Dotted object names along an import/export outer chain, root first. The
/// first name is the owning package stem; archives validate acyclicity but a
/// depth guard keeps malformed tables from looping forever.
fn outer_chain_names(archive: &PackageArchive, start: usize, import: bool) -> Option<Vec<String>> {
    let mut names = Vec::new();
    let mut cursor = start;
    loop {
        if names.len() > 256 {
            return None;
        }
        let (object_name_index, outer_ref) = if import {
            let entry = *archive.imports.get(cursor)?;
            (entry.object_name_index, entry.outer_ref)
        } else {
            let entry = *archive.exports.get(cursor)?;
            (entry.object_name_index, entry.outer_ref)
        };
        names.push(archive_name_text(archive, object_name_index)?.to_string());
        match (outer_ref, import) {
            (0, _) => return Some(names),
            (r, true) if r < 0 => cursor = (-r - 1) as usize,
            (r, false) if r > 0 => cursor = r as usize - 1,
            _ => return None,
        }
    }
}

/// Resolve an `FPoly.Texture` reference to a package-qualified key.
fn resolve_texture_key(archive: &PackageArchive, texture_ref: i32) -> Option<TextureKey> {
    let mut names = if texture_ref < 0 {
        outer_chain_names(archive, (-texture_ref - 1) as usize, true)?
    } else if texture_ref > 0 {
        outer_chain_names(archive, texture_ref as usize - 1, false)?
    } else {
        return None;
    };
    // outer_chain_names collects leaf-first; the package stem must lead.
    names.reverse();
    if names.len() < 2 {
        // A texture directly at package root has no dotted path to speak of.
        return Some(TextureKey {
            package: names.pop()?,
            object_path: String::new(),
        });
    }
    let object_path = names.split_off(1).join(".");
    Some(TextureKey {
        package: names.into_iter().next()?,
        object_path,
    })
}

/// Read the owning brush actor's placement; missing properties mean identity.
fn brush_transform(arena: &ObjectArena, level: &Level, model_outer_ref: i32) -> BrushTransform {
    const IDENTITY: BrushTransform = ([0.0; 3], [0; 3]);
    if model_outer_ref <= 0 {
        return IDENTITY;
    }
    let Some(actor_id) = level.export_ids.get(model_outer_ref as usize - 1).copied() else {
        return IDENTITY;
    };
    let Some(store) = arena.get(actor_id).ok().and_then(|o| o.properties()) else {
        return IDENTITY;
    };
    (
        struct_vec3(store, arena, "Location").unwrap_or([0.0; 3]),
        struct_ints(store, arena, "Rotation").unwrap_or([0; 3]),
    )
}

/// Extract named float fields out of a `Vector`-shaped struct property.
fn struct_vec3(store: &PropStore, arena: &ObjectArena, prop: &str) -> Option<[f32; 3]> {
    let value = store.get(arena.names.find_index(prop)?)?;
    let PropValue::Struct { fields, .. } = value else {
        return None;
    };
    let mut out = [0.0f32; 3];
    for (field_index, field_value) in fields {
        let slot = match arena
            .names
            .text(*field_index)?
            .to_ascii_uppercase()
            .as_str()
        {
            "X" => 0,
            "Y" => 1,
            "Z" => 2,
            _ => continue,
        };
        if let PropValue::Float(f) = field_value {
            out[slot] = *f;
        }
    }
    Some(out)
}

/// Extract named int fields out of a `Rotator`-shaped struct property.
fn struct_ints(store: &PropStore, arena: &ObjectArena, prop: &str) -> Option<[i32; 3]> {
    let value = store.get(arena.names.find_index(prop)?)?;
    let PropValue::Struct { fields, .. } = value else {
        return None;
    };
    let mut out = [0i32; 3];
    for (field_index, field_value) in fields {
        let slot = match arena
            .names
            .text(*field_index)?
            .to_ascii_uppercase()
            .as_str()
        {
            "PITCH" => 0,
            "YAW" => 1,
            "ROLL" => 2,
            _ => continue,
        };
        if let PropValue::Int(i) = field_value {
            out[slot] = *i;
        }
    }
    Some(out)
}

/// Rotate a direction vector by a UE1 rotator (65536 units = one turn),
/// yaw then pitch then roll, column-vector convention. Kept as one small
/// isolated function so a mirrored-world fix is a one-line flip here.
fn apply_rotator(rotation: [i32; 3], v: [f32; 3]) -> [f32; 3] {
    const UNITS_PER_TURN: f64 = 65536.0;
    let v = [v[0] as f64, v[1] as f64, v[2] as f64];
    let (sin_yaw, cos_yaw) =
        (rotation[1] as f64 / UNITS_PER_TURN * std::f64::consts::TAU).sin_cos();
    let (sin_pitch, cos_pitch) =
        (rotation[0] as f64 / UNITS_PER_TURN * std::f64::consts::TAU).sin_cos();
    let (sin_roll, cos_roll) =
        (rotation[2] as f64 / UNITS_PER_TURN * std::f64::consts::TAU).sin_cos();
    // Yaw about Z.
    let (x, y) = (
        cos_yaw * v[0] - sin_yaw * v[1],
        sin_yaw * v[0] + cos_yaw * v[1],
    );
    // Pitch about Y.
    let (x, z) = (
        cos_pitch * x + sin_pitch * v[2],
        -sin_pitch * x + cos_pitch * v[2],
    );
    // Roll about X.
    let (y, z) = (cos_roll * y - sin_roll * z, sin_roll * y + cos_roll * z);
    [x as f32, y as f32, z as f32]
}

/// Translate a point by a brush location.
fn translate(v: [f32; 3], by: [f32; 3]) -> [f32; 3] {
    [v[0] + by[0], v[1] + by[1], v[2] + by[2]]
}

/// Read three little-endian f32s.
fn read_f32x3(cur: &mut ByteCursor<'_>) -> PackageResult<[f32; 3]> {
    let mut out = [0.0f32; 3];
    for slot in &mut out {
        let bytes: [u8; 4] = cur.take(4)?.try_into().expect("four bytes");
        *slot = f32::from_le_bytes(bytes);
    }
    Ok(out)
}

/// Decode one `FPoly` (`UModel.cpp::operator<<(FArchive&, FPoly)&`).
fn parse_one_poly(cur: &mut ByteCursor<'_>) -> PackageResult<RawPoly> {
    let num_vertices = read_compact_index(cur)?;
    if !(0..=4096).contains(&num_vertices) {
        return Err(PackageError::BadLayout {
            detail: format!("absurd FPoly vertex count {num_vertices}"),
        });
    }
    let base = read_f32x3(cur)?;
    let normal = read_f32x3(cur)?;
    let tex_u = read_f32x3(cur)?;
    let tex_v = read_f32x3(cur)?;
    let mut vertices = Vec::with_capacity(num_vertices as usize);
    for _ in 0..num_vertices {
        vertices.push(read_f32x3(cur)?);
    }
    let poly_flags = cur.u32()?;
    let _actor_ref = read_compact_index(cur)?;
    let texture_ref = read_compact_index(cur)?;
    // HP2 serializes ItemName as a single compact index (no number word).
    let _item_name = read_compact_index(cur)?;
    let _i_link = read_compact_index(cur)?;
    let _i_brush_poly = read_compact_index(cur)?;
    let pan_u = i16::from_le_bytes(cur.take(2)?.try_into().expect("two bytes"));
    let pan_v = i16::from_le_bytes(cur.take(2)?.try_into().expect("two bytes"));
    Ok(RawPoly {
        base,
        normal,
        tex_u,
        tex_v,
        vertices,
        poly_flags,
        texture_ref,
        pan_uv: [pan_u as f32, pan_v as f32],
    })
}

/// Decode a `UPolys` payload: optional tagged-property prologue, then
/// `(DbNum, DbMax)`, then `DbNum` brush polygons.
fn parse_upolys(payload: &[u8], archive: &PackageArchive) -> Result<Vec<RawPoly>> {
    // Most payloads carry an empty tagged-property prologue (one terminator
    // byte); a handful ship none at all. Try both alignments.
    if let Ok(polys) = parse_upolys_at(payload, archive, true) {
        return Ok(polys);
    }
    parse_upolys_at(payload, archive, false)
        .map_err(|error| EngineError::new("engine.scene_polys", error.to_string()))
}

fn parse_upolys_at(
    payload: &[u8],
    archive: &PackageArchive,
    skip_tags: bool,
) -> PackageResult<Vec<RawPoly>> {
    let decoded: PackageResult<Vec<RawPoly>> = (|| {
        let mut cur = ByteCursor::new(payload);
        if skip_tags {
            read_property_tags(&mut cur, &archive.names)?;
        }
        let count = cur.i32()?;
        let _db_max = cur.i32()?;
        if count < 0 {
            return Err(PackageError::BadLayout {
                detail: format!("negative UPolys count {count}"),
            });
        }
        let mut polys = Vec::with_capacity(count as usize);
        for _ in 0..count {
            polys.push(parse_one_poly(&mut cur)?);
        }
        // Alignment gate: a correct decode consumes the payload exactly.
        if !polys.is_empty() && cur.position() != payload.len() {
            return Err(PackageError::BadLayout {
                detail: format!(
                    "UPolys trailing bytes: {} of {} consumed",
                    cur.position(),
                    payload.len()
                ),
            });
        }
        Ok(polys)
    })();
    decoded
}

/// Object reference a `UModel` export payload carries for its `Polys`
/// subobject. Per `UModel::Serialize` (Ver>61): tagged-property prologue,
/// then compact signed refs for Vectors/Points/Nodes/Surfs/Verts, raw
/// `NumSharedSides`, raw `NumZones`, `NumZones` × `FZoneProperties`
/// (compact ref, u64 connectivity, u64 visibility — `LastRenderTime` is
/// dead code in the C++ serializer), then the `Polys` object ref.
fn model_polys_ref(payload: &[u8], archive: &PackageArchive) -> Result<i32> {
    const MAX_ZONES: i32 = 64;
    let decoded: PackageResult<i32> = (|| {
        let mut cur = ByteCursor::new(payload);
        read_property_tags(&mut cur, &archive.names)?;
        for _ in 0..5 {
            read_compact_index(&mut cur)?;
        }
        let _num_shared_sides = cur.i32()?;
        let num_zones = cur.i32()?;
        if !(0..=MAX_ZONES).contains(&num_zones) {
            return Err(PackageError::BadLayout {
                detail: format!("absurd UModel zone count {num_zones}"),
            });
        }
        for _ in 0..num_zones {
            let _zone_actor = read_compact_index(&mut cur)?;
            let _connectivity: [u8; 8] = cur.take(8)?.try_into().expect("eight bytes");
            let _visibility: [u8; 8] = cur.take(8)?.try_into().expect("eight bytes");
        }
        read_compact_index(&mut cur)
    })();
    decoded.map_err(|error| EngineError::new("engine.scene_model", error.to_string()))
}

/// Class label of an arena object (`PlayerStart`, `Brush`, ...).
fn class_label(arena: &ObjectArena, id: ObjectId) -> &str {
    arena
        .get(id)
        .ok()
        .and_then(|object| object.class_id)
        .and_then(|class_id| arena.get(class_id).ok())
        .and_then(|class| arena.names.text(class.name_index))
        .unwrap_or("")
}

/// Camera seed: first `PlayerStart` actor, else the first `Camera` actor.
fn find_camera(arena: &ObjectArena, level: &Level) -> Option<SceneCamera> {
    for wanted in ["PlayerStart", "Camera"] {
        for id in &level.actors {
            if !class_label(arena, *id).eq_ignore_ascii_case(wanted) {
                continue;
            }
            let store = arena.get(*id).ok()?.properties()?;
            return Some(SceneCamera {
                location: struct_vec3(store, arena, "Location").unwrap_or([0.0; 3]),
                rotation: struct_ints(store, arena, "Rotation").unwrap_or([0; 3]),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn privet_archive() -> Option<hp_format::package79::PackageArchive> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        if !root.join("Maps/PrivetDr.unr").is_file() {
            return None;
        }
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).ok()?;
        hp_format::package79::read_package(&bytes).ok()
    }

    /// First `UPolys` export of the retail map decodes as 6 four-vertex
    /// brush polygons with texture references (wire format verified against
    /// `UModel.cpp::operator<<(FArchive&, FPoly&)`).
    #[test]
    fn privet_dr_upolys_payload_decodes() {
        let Some(archive) = privet_archive() else {
            return;
        };
        let (index, _) = archive
            .exports
            .iter()
            .enumerate()
            .find(|(_, e)| ref_class_text(&archive, e.class_ref) == Some("Polys"))
            .expect("map carries Polys exports");
        let payload = archive.export_payload(index).unwrap();
        let polys = parse_upolys(payload, &archive).expect("UPolys parses");
        assert_eq!(polys.len(), 6, "Polys76 holds six brush polygons");
        for poly in &polys {
            assert_eq!(poly.vertices.len(), 4);
        }
        assert!(
            polys.iter().any(|p| p.texture_ref != 0),
            "brush polys carry texture references"
        );
    }

    /// Texture references resolve to shipped .utx package stems.
    #[test]
    fn texture_keys_resolve_to_known_packages() {
        let Some(archive) = privet_archive() else {
            return;
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut parsed = 0usize;
        for (index, entry) in archive.exports.iter().enumerate() {
            let Some(class_text) = ref_class_text(&archive, entry.class_ref) else {
                continue;
            };
            if class_text != "Polys" {
                continue;
            }
            let payload = archive.export_payload(index).unwrap();
            // A minority of UPolys payloads use a layout variant we do not
            // decode yet; the census runs over everything that parses.
            let Ok(polys) = parse_upolys(payload, &archive) else {
                continue;
            };
            parsed += 1;
            for poly in polys {
                if let Some(key) = resolve_texture_key(&archive, poly.texture_ref) {
                    seen.insert(key.package.to_ascii_uppercase());
                }
            }
        }
        assert!(
            parsed >= 20,
            "expected most Polys exports to parse, got {parsed}"
        );
        assert!(
            seen.contains("HP2_MASTER") || seen.contains("PRIVETDRIVE"),
            "expected shipped texture packages, got {seen:?}"
        );
    }

    /// Rotator math: identity keeps vectors; quarter yaw moves X onto Y.
    #[test]
    fn rotator_basics() {
        let v = apply_rotator([0, 0, 0], [1.0, 0.0, 0.0]);
        assert!((v[0] - 1.0).abs() < 1e-5);
        let v = apply_rotator([0, 16384, 0], [1.0, 0.0, 0.0]); // 90 degrees yaw
        assert!((v[0]).abs() < 1e-5 && (v[1] - 1.0).abs() < 1e-5);
    }

    /// Missing archive => loud reason-coded failure, not a silent empty scene.
    #[test]
    fn bootstrap_placeholder_is_loud() {
        let arena = hp_uobject::arena::ObjectArena::new();
        let level = crate::level::Level {
            root: hp_uobject::arena::ObjectId(u32::MAX),
            actors: Vec::new(),
            model_count: 0,
            export_ids: Vec::new(),
            archive: None,
        };
        let error = build_render_scene(&arena, &level).unwrap_err();
        assert_eq!(error.reason_code, "engine.scene_no_map");
    }
}

#[cfg(test)]
mod linkprobe {
    use super::*;
    use std::path::PathBuf;
    fn try_ci(buf: &[u8], pos: usize) -> hp_format::package79::PackageResult<(i32, usize)> {
        let mut cur = ByteCursor::new(buf);
        cur.seek(pos);
        let v = read_compact_index(&mut cur)?;
        Ok((v, cur.position()))
    }
    #[test]
    fn offset55_hits_all_models() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).unwrap();
        let archive = hp_format::package79::read_package(&bytes).unwrap();
        let polys_set: std::collections::HashSet<usize> = archive
            .exports
            .iter()
            .enumerate()
            .filter(|(_, e)| ref_class_text(&archive, e.class_ref) == Some("Polys"))
            .map(|(i, _)| i)
            .collect();
        let mut small_total = 0;
        let mut small_hit = 0;
        let mut big = 0;

        for (index, entry) in archive.exports.iter().enumerate() {
            if ref_class_text(&archive, entry.class_ref) != Some("Model") {
                continue;
            }
            let Some(payload) = archive.export_payload(index) else {
                continue;
            };
            if payload.len() > 10000 {
                big += 1;
                continue;
            } // level model
            small_total += 1;
            let mut hit = false;
            for off in 45..=65usize {
                if off >= payload.len() {
                    break;
                }
                if let Ok((v, _)) = try_ci(payload, off)
                    && v > 0
                    && polys_set.contains(&(v as usize - 1))
                {
                    hit = true;
                    break;
                }
            }
            if hit {
                small_hit += 1;
            }
        }
        println!("small_models={small_total} windowed_polys_hits={small_hit}");
        println!("big(level)models={big}");
    }
}

#[cfg(test)]
mod actorprobe {
    use super::*;
    use std::path::PathBuf;

    /// One HP2 FPropertyTag walk attempt from `start`. Returns Ok(offsets of
    /// Vector struct values) when the stream terminates on NAME_None without
    /// erroring before payload end.
    fn walk(payload: &[u8], archive: &PackageArchive, start: usize) -> Option<Vec<[f32; 3]>> {
        let mut cur = ByteCursor::new(payload);
        cur.seek(start);
        let mut vectors = Vec::new();
        loop {
            if cur.position() >= payload.len() {
                return None;
            }
            let name = read_compact_index(&mut cur).ok()?;
            // NAME_None terminator: index 0 per UE1 convention.
            if name == 0 {
                return if cur.position() <= payload.len() {
                    Some(vectors)
                } else {
                    None
                };
            }
            let info = cur.u8().ok()?;
            let ty = info & 0x0f;
            let mut item_name = None;
            if ty == 8 {
                item_name = Some(read_compact_index(&mut cur).ok()?);
            } // StructProperty ItemName
            let name_text = archive_name_text(archive, name).unwrap_or("?").to_string();
            println!("        TAG start={start} name={name}({name_text}) info={info:02x} ty={ty}");
            let size = match info & 0x70 {
                0x00 => 1,
                0x10 => 2,
                0x20 => 4,
                0x30 => 12,
                0x40 => 16,
                0x50 => cur.u8().ok()? as usize,
                0x60 => u16::from_le_bytes(cur.take(2).ok()?.try_into().ok()?) as usize,
                0x70 => cur.i32().ok()? as usize,
                _ => return None,
            };
            let _array_index = if (info & 0x80) != 0 && ty != 3 {
                read_compact_index(&mut cur).ok()?
            } else {
                0
            };
            match ty {
                4 => {
                    let f = f32::from_le_bytes(cur.take(4).ok()?.try_into().ok()?);
                    vectors.push([f, 0.0, 0.0]);
                }
                8 => {
                    let item_text = archive_name_text(archive, item_name.unwrap_or(0))
                        .unwrap_or("?")
                        .to_string();
                    if size == 12 {
                        let mut f = [0f32; 3];
                        for slot in &mut f {
                            *slot = f32::from_le_bytes(cur.take(4).ok()?.try_into().ok()?);
                        }
                        println!("        STRUCT {item_text} = {f:?}");
                    } else {
                        let _ = cur.take(size).ok()?;
                    }
                }
                _ => {
                    let _ = cur.take(size).ok()?;
                }
            }
        }
    }

    #[test]
    fn calibrate_playerstart_walk() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).unwrap();
        let archive = hp_format::package79::read_package(&bytes).unwrap();
        for (index, entry) in archive.exports.iter().enumerate() {
            if ref_class_text(&archive, entry.class_ref) != Some("PlayerStart") {
                continue;
            }
            let payload = archive.export_payload(index).unwrap();
            for start in 0..30usize {
                if let Some(vs) = walk(payload, &archive, start) {
                    println!("START={start} terminated, vectors={vs:?}");
                }
            }
            break;
        }
    }
}
