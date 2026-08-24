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
    let mut brushes = 0usize;
    let mut linked = 0usize;
    let mut referenced_models: std::collections::HashSet<usize> = Default::default();

    // One model export's UPolys -> world-space scene polys under the given
    // brush transform. Returns the poly count.
    let push_model_polys = |archive: &PackageArchive,
                            model_index: usize,
                            location: [f32; 3],
                            rotation: [i32; 3],
                            scale: [f32; 3],
                            polys: &mut Vec<ScenePoly>|
     -> Result<usize> {
        let entry = archive
            .exports
            .get(model_index)
            .ok_or_else(|| EngineError::new("engine.scene_model", "model export missing"))?;
        if ref_class_text(archive, entry.class_ref) != Some("Model") {
            return Err(EngineError::new("engine.scene_model", "not a Model export"));
        }
        let payload = archive
            .export_payload(model_index)
            .ok_or_else(|| EngineError::new("engine.scene_model", "model has no payload"))?;
        let polys_ref = model_polys_ref(payload, archive, entry.class_ref)?;
        if polys_ref <= 0 {
            return Err(EngineError::new("engine.scene_model", "no Polys ref"));
        }
        let polys_payload = archive
            .export_payload(polys_ref as usize - 1)
            .ok_or_else(|| EngineError::new("engine.scene_model", "Polys payload missing"))?;
        let parsed = parse_upolys(polys_payload, archive)?;
        let count = parsed.len();
        for raw_poly in parsed {
            polys.push(ScenePoly {
                base: transform_vertex(location, rotation, scale, raw_poly.base),
                vertices: raw_poly
                    .vertices
                    .iter()
                    .map(|v| transform_vertex(location, rotation, scale, *v))
                    .collect(),
                normal: rotate_only(rotation, raw_poly.normal),
                tex_u: rotate_only(rotation, raw_poly.tex_u),
                tex_v: rotate_only(rotation, raw_poly.tex_v),
                pan_uv: raw_poly.pan_uv,
                poly_flags: raw_poly.poly_flags,
                texture: resolve_texture_key(archive, raw_poly.texture_ref),
            });
        }
        Ok(count)
    };

    for &actor in &level.actors {
        if !class_label(arena, actor).eq_ignore_ascii_case("Brush") {
            continue;
        }
        let Ok(obj) = arena.get(actor) else { continue };
        let Some(store) = obj.properties() else {
            continue;
        };
        let location = struct_vec3(store, arena, "Location").unwrap_or([0.0; 3]);
        let rotation = struct_ints(store, arena, "Rotation").unwrap_or([0; 3]);
        let scale = struct_vec3(store, arena, "MainScale").unwrap_or([1.0; 3]);
        // 'Brush' object property -> Model export (raw package ref).
        let brush_prop = arena.names.find_index("Brush").and_then(|n| store.get(n));
        let Some(PropValue::Object(Some(raw))) = brush_prop else {
            continue;
        };
        let raw = *raw;
        if raw <= 0 {
            continue;
        }
        let model_index = raw as usize - 1;
        if archive.exports.get(model_index).is_none() {
            continue;
        }
        brushes += 1;
        referenced_models.insert(model_index);
        if push_model_polys(archive, model_index, location, rotation, scale, &mut polys).is_ok() {
            linked += 1;
        }
    }

    // The static level geometry: the CSG'd level model is world-space and is
    // owned by the level itself, not by any Brush actor. Every Model export
    // no brush references carries level (or orphaned) geometry — add it with
    // an identity transform.
    let mut level_models = 0usize;
    for index in 0..archive.exports.len() {
        if referenced_models.contains(&index) {
            continue;
        }
        let entry = &archive.exports[index];
        if ref_class_text(archive, entry.class_ref) != Some("Model") {
            continue;
        }
        let payload = match archive.export_payload(index) {
            Some(payload) => payload,
            None => continue,
        };
        let parsed = match parse_model(payload, archive, entry.class_ref) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!(
                    "hp-engine: note [engine.scene_level_model] model export {index}: {error}"
                );
                continue;
            }
        };
        if parsed.nodes.is_empty() {
            // No BSP topology — fall back to the Polys path (identity brush).
            match push_model_polys(archive, index, [0.0; 3], [0; 3], [1.0; 3], &mut polys) {
                Ok(_) => level_models += 1,
                Err(error) => {
                    eprintln!(
                        "hp-engine: note [engine.scene_level_model] model export {index}: {error}"
                    );
                }
            }
            continue;
        }
        let before = polys.len();
        push_bsp_polys(&parsed, archive, &mut polys);
        level_models += 1;
        println!(
            "hp-engine: level model {index}: {} nodes -> {} polys",
            parsed.nodes.len(),
            polys.len() - before
        );
    }
    println!(
        "hp-engine: scene extraction brushes={brushes} linked={linked} level_models={level_models} polys={}",
        polys.len()
    );

    Ok(RenderScene {
        polys,
        camera: find_camera(arena, level),
    })
}

/// Build world-space scene polys from a CSG'd level model's BSP topology:
/// each node's vertex pool slices the shared `verts` pool into `points`
/// positions; its surface carries the texture binding and UV basis (indices
/// into the model's Vectors/Points). Mirrors openhp1 `Model::triangulate`.
fn push_bsp_polys(model: &ParsedModel, archive: &PackageArchive, polys: &mut Vec<ScenePoly>) {
    for node in &model.nodes {
        if node.vertex_count < 3 {
            continue;
        }
        if node.surface < 0 {
            continue;
        }
        let Some(surf) = model.surfs.get(node.surface as usize) else {
            continue;
        };
        let pool_end = node.vertex_pool + node.vertex_count as usize;
        if pool_end > model.verts.len() {
            continue;
        }
        let Some(base) = model.points.get(surf.base_point).copied() else {
            continue;
        };
        let (Some(tex_u), Some(tex_v), Some(normal)) = (
            model.vectors.get(surf.tex_u).copied(),
            model.vectors.get(surf.tex_v).copied(),
            model.vectors.get(surf.normal).copied(),
        ) else {
            continue;
        };
        let vertices: Vec<[f32; 3]> = model.verts[node.vertex_pool..pool_end]
            .iter()
            .filter_map(|&p| model.points.get(p).copied())
            .collect();
        if vertices.len() < 3 {
            continue;
        }
        polys.push(ScenePoly {
            base,
            vertices,
            normal,
            tex_u,
            tex_v,
            pan_uv: [surf.pan_u as f32, surf.pan_v as f32],
            poly_flags: surf.poly_flags,
            texture: resolve_texture_key(archive, surf.texture_ref),
        });
    }
}

/// Brush-local point -> world: MainScale, then rotator, then Location.
fn transform_vertex(
    location: [f32; 3],
    rotation: [i32; 3],
    scale: [f32; 3],
    v: [f32; 3],
) -> [f32; 3] {
    let scaled = [v[0] * scale[0], v[1] * scale[1], v[2] * scale[2]];
    translate(apply_rotator(rotation, scaled), location)
}

/// Direction vector -> world (rotation only).
fn rotate_only(rotation: [i32; 3], v: [f32; 3]) -> [f32; 3] {
    apply_rotator(rotation, v)
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

/// Decode a `UPolys` payload under any observed retail alignment:
/// optional prologue/terminator, then `(DbNum, DbMax)`, then `DbNum`
/// brush polygons.
fn parse_upolys(payload: &[u8], archive: &PackageArchive) -> Result<Vec<RawPoly>> {
    // Alignments seen in retail maps: HP2 object-stack prologue (payload
    // bit 0x80 set: [ci][ci][FF×8][i32][u8 0x81]), tagged-property
    // terminator only, no prologue at all, or a small constant offset.
    // Prefer the first NON-EMPTY exact-consumption decode; a wrong
    // alignment can legitimately decode zero polys and must not mask the
    // right one.
    let mut fallback: Option<Vec<RawPoly>> = None;
    if payload.first().is_some_and(|b| b & 0x80 != 0) {
        let mut cur = ByteCursor::new(payload);
        if skip_upolys_prologue(&mut cur) {
            match parse_upolys_stream(&mut cur, payload) {
                Ok(polys) if !polys.is_empty() => return Ok(polys),
                Ok(polys) => fallback = fallback.or(Some(polys)),
                Err(_) => {}
            }
        }
    }
    for skip_tags in [true, false] {
        if let Ok(polys) = parse_upolys_at(payload, archive, skip_tags) {
            if !polys.is_empty() {
                return Ok(polys);
            }
            fallback = fallback.or(Some(polys));
        }
    }
    for start in 1usize..48 {
        if start >= payload.len() {
            break;
        }
        let mut cur = ByteCursor::at(payload, start);
        if let Ok(polys) = parse_upolys_from(&mut cur, payload) {
            if !polys.is_empty() {
                return Ok(polys);
            }
            fallback = fallback.or(Some(polys));
        }
    }
    fallback.ok_or_else(|| {
        EngineError::new(
            "engine.scene_polys",
            "no UPolys alignment consumed the payload exactly",
        )
    })
}

/// Skip the HP2 object-stack prologue on a `UPolys` payload:
/// `[ci][ci][FF×8][i32][u8 0x81]` (wire spec item 1). Restores the cursor
/// and returns false when the marker does not match.
fn skip_upolys_prologue(cur: &mut ByteCursor<'_>) -> bool {
    let save = cur.position();
    if read_compact_index(cur).is_err() || read_compact_index(cur).is_err() {
        cur.seek(save);
        return false;
    }
    let ff8 = cur
        .take(8)
        .is_ok_and(|b| b.len() == 8 && b.iter().all(|&x| x == 0xff));
    let ok = ff8 && cur.i32().is_ok() && cur.u8().is_ok_and(|m| m == 0x81);
    if !ok {
        cur.seek(save);
    }
    ok
}

fn parse_upolys_at(
    payload: &[u8],
    archive: &PackageArchive,
    skip_tags: bool,
) -> PackageResult<Vec<RawPoly>> {
    let mut cur = ByteCursor::new(payload);
    if skip_tags {
        read_property_tags(&mut cur, &archive.names)?;
    }
    let _ = archive;
    parse_upolys_stream(&mut cur, payload)
}

fn parse_upolys_from(cur: &mut ByteCursor<'_>, payload: &[u8]) -> Result<Vec<RawPoly>> {
    parse_upolys_stream(cur, payload)
        .map_err(|error| EngineError::new("engine.scene_polys", error.to_string()))
}

fn parse_upolys_stream(cur: &mut ByteCursor<'_>, payload: &[u8]) -> PackageResult<Vec<RawPoly>> {
    let count = cur.i32()?;
    let _db_max = cur.i32()?;
    if !(0..=4096).contains(&count) {
        return Err(PackageError::BadLayout {
            detail: format!("implausible UPolys count {count}"),
        });
    }
    let mut polys = Vec::with_capacity(count as usize);
    for _ in 0..count {
        polys.push(parse_one_poly(cur)?);
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
}

/// One decoded `FBspNode` record: the fields scene extraction needs.
#[derive(Debug, Clone)]
pub(crate) struct BspNodeData {
    pub vertex_pool: usize,
    pub vertex_count: u8,
    pub surface: i32,
}

/// One decoded `FBspSurf` record: texture binding and UV basis as indices
/// into the model's Vectors/Points arrays.
#[derive(Debug, Clone)]
pub(crate) struct BspSurfData {
    pub texture_ref: i32,
    pub poly_flags: u32,
    pub base_point: usize,
    pub normal: usize,
    pub tex_u: usize,
    pub tex_v: usize,
    pub pan_u: i16,
    pub pan_v: i16,
}

/// A fully decoded `UModel` payload: the BSP topology arrays plus the
/// `Polys` subobject reference. Brush models carry empty BSP arrays and all
/// geometry in `Polys`; the CSG'd level model carries its world geometry in
/// the BSP arrays and an (often empty) `Polys`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ParsedModel {
    pub vectors: Vec<[f32; 3]>,
    pub points: Vec<[f32; 3]>,
    pub nodes: Vec<BspNodeData>,
    pub surfs: Vec<BspSurfData>,
    /// Vertex pool: `FVert.pVertex` index into `points` (the `iSide` is
    /// topology-only and unused for rendering).
    pub verts: Vec<usize>,
    pub polys_ref: i32,
}

pub(crate) fn model_polys_ref(
    payload: &[u8],
    archive: &PackageArchive,
    class_ref: i32,
) -> Result<i32> {
    parse_model(payload, archive, class_ref).map(|model| model.polys_ref)
}

/// Decode one `UModel` export payload (grammar per `UModel::Serialize`,
/// HP2 deltas validated by exact consumption across the shipped maps).
pub(crate) fn parse_model(
    payload: &[u8],
    _archive: &PackageArchive,
    class_ref: i32,
) -> Result<ParsedModel> {
    let decoded: PackageResult<ParsedModel> = (|| {
        let mut cur = ByteCursor::new(payload);
        crate::level::skip_object_header(&mut cur, class_ref);
        // Tagged-property terminator ("None"); models carry no properties.
        let _none = read_compact_index(&mut cur)?;
        // Bounds FBox (6 x f32).
        for _ in 0..6 {
            let _ = read_f32_scalar(&mut cur)?;
        }
        let _a = cur.i32()?;
        let _b9_reserved = cur.take(9)?;
        let _c_radius = read_f32_scalar(&mut cur)?;
        let mut model = ParsedModel::default();
        // Vectors / Points TArray<FVector>.
        for sink in [&mut model.vectors, &mut model.points] {
            let n = read_compact_index(&mut cur)?;
            sink.reserve(n.max(0) as usize);
            for _ in 0..n {
                sink.push(read_f32x3(&mut cur)?);
            }
        }

        // Nodes TArray<FBspNode>: Plane f32x4, ZoneMask u32, NodeFlags u32,
        // 1 reserved byte, then SEVEN compact refs (iVertPool, iSurf, back,
        // front, plane, collision_bound, render_bound), iZone 2 bytes,
        // NumVertices u8, iLeaf 2 x i32. Layout brute-force-validated on
        // Model1 (3306 nodes: unique per-node pools, surf refs in range,
        // exact stream alignment).
        let nodes = read_compact_index(&mut cur)?;
        model.nodes.reserve(nodes.max(0) as usize);
        for _ in 0..nodes {
            for _ in 0..4 {
                let _ = read_f32_scalar(&mut cur)?;
            }
            let _zone_mask_and_flags: [u8; 9] =
                cur.take(9)?
                    .try_into()
                    .map_err(|_| PackageError::BadLayout {
                        detail: "node zone mask".into(),
                    })?;
            let mut refs = [0i32; 7];
            for slot in &mut refs {
                *slot = read_compact_index(&mut cur)?;
            }
            let _izones: [u8; 2] =
                cur.take(2)?
                    .try_into()
                    .map_err(|_| PackageError::BadLayout {
                        detail: "node izones".into(),
                    })?;
            let vertex_count = cur.u8()?;
            let _leaf_back = cur.i32()?;
            let _leaf_front = cur.i32()?;
            model.nodes.push(BspNodeData {
                vertex_pool: refs[0].max(0) as usize,
                vertex_count,
                surface: refs[1],
            });
        }
        // Surfs TArray<FBspSurf>: compact texture ref, u32 PolyFlags, six
        // compact refs (base_point, normal, tex_u, tex_v, light_map,
        // brush_poly), i16 PanU/PanV, compact actor ref.
        let surfs = read_compact_index(&mut cur)?;
        model.surfs.reserve(surfs.max(0) as usize);
        for _ in 0..surfs {
            let texture = read_compact_index(&mut cur)?;
            let poly_flags = cur.u32()?;
            let mut refs = [0i32; 6];
            for slot in &mut refs {
                *slot = read_compact_index(&mut cur)?;
            }
            let pan_u = i16::from_le_bytes(cur.take(2)?.try_into().map_err(|_| {
                PackageError::BadLayout {
                    detail: "surf pan u".into(),
                }
            })?);
            let pan_v = i16::from_le_bytes(cur.take(2)?.try_into().map_err(|_| {
                PackageError::BadLayout {
                    detail: "surf pan v".into(),
                }
            })?);
            let _actor = read_compact_index(&mut cur)?;
            model.surfs.push(BspSurfData {
                texture_ref: texture,
                poly_flags,
                base_point: refs[0].max(0) as usize,
                normal: refs[1].max(0) as usize,
                tex_u: refs[2].max(0) as usize,
                tex_v: refs[3].max(0) as usize,
                pan_u,
                pan_v,
            });
        }
        // Verts TArray<FVert>: two compact indices each.
        let verts = read_compact_index(&mut cur)?;
        model.verts.reserve(verts.max(0) as usize);
        for _ in 0..verts {
            let p_vertex = read_compact_index(&mut cur)?;
            let _i_side = read_compact_index(&mut cur)?;
            model.verts.push(p_vertex.max(0) as usize);
        }
        let _num_shared_sides = cur.i32()?;
        let num_zones = cur.i32()?;
        if !(0..=64).contains(&num_zones) {
            return Err(PackageError::BadLayout {
                detail: format!("absurd UModel zone count {num_zones}"),
            });
        }
        for _ in 0..num_zones {
            let _zone_actor = read_compact_index(&mut cur)?;
            let conn: [u8; 8] = cur
                .take(8)?
                .try_into()
                .map_err(|_| PackageError::BadLayout {
                    detail: "zone connectivity".into(),
                })?;
            let vis: [u8; 8] = cur
                .take(8)?
                .try_into()
                .map_err(|_| PackageError::BadLayout {
                    detail: "zone visibility".into(),
                })?;
            let _ = (conn, vis);
        }
        // THE LINKAGE FIELD.
        model.polys_ref = read_compact_index(&mut cur)?;
        Ok(model)
    })();
    decoded.map_err(|error| EngineError::new("engine.scene_model", error.to_string()))
}

/// Read one little-endian f32 scalar.
fn read_f32_scalar(cur: &mut ByteCursor<'_>) -> PackageResult<f32> {
    let b: [u8; 4] = cur
        .take(4)?
        .try_into()
        .map_err(|_| PackageError::BadLayout {
            detail: "f32".into(),
        })?;
    Ok(f32::from_le_bytes(b))
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

    #[test]
    fn giant_poly_vertices() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let mut world = hp_uobject::bootstrap::World::load(&root).unwrap();
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).unwrap();
        let level =
            crate::level::load_level_from_bytes(&mut world.arena, "PrivetDr", &bytes).unwrap();
        let scene = build_render_scene(&world.arena, &level).unwrap();
        for i in [883usize, 884, 2268] {
            let p = &scene.polys[i];
            println!("poly#{i} base={:?} normal={:?}", p.base, p.normal);
            for v in &p.vertices {
                println!("  v {v:?}");
            }
            println!("  tex_u={:?} tex_v={:?}", p.tex_u, p.tex_v);
        }
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

#[cfg(test)]
mod structprobe {
    #[test]
    fn vector_struct_fields() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let world = hp_uobject::bootstrap::World::load(&root).unwrap();
        let arena = &world.arena;
        let obj16 = arena.get(hp_uobject::arena::ObjectId(16)).unwrap();
        println!(
            "ObjectId(16) name={:?} data_is_script_struct={}",
            arena.names.text(obj16.name_index).unwrap_or("?"),
            matches!(obj16.data, hp_uobject::arena::ObjectData::ScriptStruct(_))
        );
        if let Ok(Some((sname, fields))) =
            hp_uobject::loader::flatten_struct_fields(arena, hp_uobject::arena::ObjectId(16))
        {
            println!(
                "struct {:?} fields={}",
                arena.names.text(sname).unwrap_or("?"),
                fields.len()
            );
        } else {
            println!("flatten returned None");
        }
        // find the real Vector script struct by name
        for (id, o) in arena.objects_iter() {
            if let hp_uobject::arena::ObjectData::ScriptStruct(_) = o.data {
                let t = arena.names.text(o.name_index).unwrap_or("");
                if t.eq_ignore_ascii_case("Vector") {
                    println!("Vector struct at ObjectId({:?})", id.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod brushprobe {
    use super::*;
    #[test]
    fn brush_store_keys() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let mut world = hp_uobject::bootstrap::World::load(&root).unwrap();
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).unwrap();
        let level =
            crate::level::load_level_from_bytes(&mut world.arena, "PrivetDr", &bytes).unwrap();
        let arena = &world.arena;
        let mut shown = 0;
        for &id in &level.actors {
            if !class_label(arena, id).eq_ignore_ascii_case("Brush") {
                continue;
            }
            if shown >= 2 {
                break;
            }
            shown += 1;
            let obj = arena.get(id).unwrap();
            println!(
                "== brush {:?} props:",
                arena.display_name(id).unwrap_or_default()
            );
            {
                let arch = level.archive.as_ref().unwrap();
                // find its model export payload via raw ref
                if let Some(store) = obj.properties() {
                    println!("   store_len={}", store.len());
                }
                let _ = arch;
            }
            if let Some(store) = obj.properties() {
                for (n, v) in store.iter() {
                    println!("   {} = {:?}", arena.names.text(n).unwrap_or("?"), v);
                }
            } else {
                println!("   <no store>")
            }
        }
        // also PlayerStart
        for &id in &level.actors {
            if !class_label(arena, id).eq_ignore_ascii_case("PlayerStart") {
                continue;
            }
            let obj = arena.get(id).unwrap();
            if let Some(store) = obj.properties() {
                for (n, v) in store.iter() {
                    println!("PS {} = {:?}", arena.names.text(n).unwrap_or("?"), v);
                }
            }
            break;
        }
    }
}

#[cfg(test)]
mod tagprobe {
    use super::*;
    use hp_format::package79::ByteCursor;
    #[test]
    fn brush_tags_dump() {
        let root =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let bytes = std::fs::read(root.join("Maps/PrivetDr.unr")).unwrap();
        let archive = hp_format::package79::read_package(&bytes).unwrap();
        // find export named Brush423
        for (i, e) in archive.exports.iter().enumerate() {
            if archive_name_text(&archive, e.object_name_index) == Some("Brush423") {
                let payload = archive.export_payload(i).unwrap();
                let mut cur = ByteCursor::at(payload, 0);
                crate::level::skip_object_header(&mut cur, e.class_ref);
                println!("after header pos={} len={}", cur.position(), payload.len());
                match crate::level::read_actor_tags(&mut cur, &archive.names) {
                    Ok(tags) => {
                        for t in &tags {
                            println!(
                                "tag {} kind={} size={} payload={:02x?}",
                                archive
                                    .names
                                    .get(t.name_index as usize)
                                    .map(|n| n.text.as_str())
                                    .unwrap_or("?"),
                                t.kind,
                                t.payload.len(),
                                &t.payload[..t.payload.len().min(16)]
                            );
                        }
                    }
                    Err(e) => println!("ERR {e}"),
                }
                break;
            }
        }
    }
}

/// G4 structural acceptance: numeric correctness of scene extraction,
/// per the phase gate. These tests encode the wire facts directly and do
/// not depend on pixels.
#[cfg(test)]
mod g4_structural {
    use super::*;
    use std::path::PathBuf;

    struct Loaded {
        world: hp_uobject::bootstrap::World,
        level: Level,
        scene: RenderScene,
    }

    fn load(map: &str) -> Loaded {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let mut world = hp_uobject::bootstrap::World::load(&root).unwrap();
        let bytes = std::fs::read(root.join(format!("Maps/{map}.unr"))).unwrap();
        let level = crate::level::load_level_from_bytes(&mut world.arena, map, &bytes).unwrap();
        let scene = build_render_scene(&world.arena, &level).unwrap();
        Loaded {
            world,
            level,
            scene,
        }
    }

    /// Independent brush-transform reimplementation (documented rule:
    /// world = Location + R(yaw,pitch,roll) · (MainScale ∘ v)) used to
    /// cross-check `transform_vertex`.
    fn hand_transform(
        location: [f32; 3],
        rotation: [i32; 3],
        scale: [f32; 3],
        v: [f32; 3],
    ) -> [f32; 3] {
        let rad = |u: i32| u as f32 * std::f32::consts::TAU / 65536.0;
        let (sy, cy) = rad(rotation[1]).sin_cos();
        let (sp, cp) = rad(rotation[0]).sin_cos();
        let (sr, cr) = rad(rotation[2]).sin_cos();
        let s = [v[0] * scale[0], v[1] * scale[1], v[2] * scale[2]];
        // Rz(yaw) · Ry(pitch) · Rx(roll), column-vector convention.
        let x = cy * cp * s[0] + (cy * sp * sr - sy * cr) * s[1] + (cy * sp * cr + sy * sr) * s[2];
        let y = sy * cp * s[0] + (sy * sp * sr + cy * cr) * s[1] + (sy * sp * cr - cy * sr) * s[2];
        let z = -sp * s[0] + cp * sr * s[1] + cp * cr * s[2];
        [x + location[0], y + location[1], z + location[2]]
    }

    fn close(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
        a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() <= eps)
    }

    /// (1) Brush transforms: hand-computed world vertices for five brushes
    /// (including Brush423 with non-default placement) must appear in the
    /// scene output.
    #[test]
    fn brush_transforms_match_hand_computed_world_vertices() {
        let Loaded {
            world,
            level,
            scene,
        } = load("PrivetDr");
        let arena = &world.arena;
        let archive = level.archive.as_ref().unwrap();

        let mut checked = 0usize;
        let mut matched_polys = 0usize;
        let mut saw_423 = false;
        let mut total_polys = 0usize;
        for &actor in &level.actors {
            if !class_label(arena, actor).eq_ignore_ascii_case("Brush") {
                continue;
            }
            let name = arena.display_name(actor).unwrap_or_default().to_string();
            if name == "Brush423" {
                saw_423 = true;
            } else if checked >= 4 {
                continue;
            }
            let obj = arena.get(actor).unwrap();
            let Some(store) = obj.properties() else {
                continue;
            };
            let location = struct_vec3(store, arena, "Location").unwrap();
            let rotation = struct_ints(store, arena, "Rotation").unwrap_or([0; 3]);
            let scale = struct_vec3(store, arena, "MainScale").unwrap_or([1.0; 3]);
            if name == "Brush423" {
                // Documented retail placement for Brush423.
                assert!(
                    close(location, [7760.0, 352.0, 208.0], 0.5),
                    "Brush423 Location {location:?}"
                );
            }
            let Some(PropValue::Object(Some(raw))) =
                arena.names.find_index("Brush").and_then(|n| store.get(n))
            else {
                continue;
            };
            let model_index = *raw as usize - 1;
            let payload = archive.export_payload(model_index).unwrap();
            let model = parse_model(payload, archive, archive.exports[model_index].class_ref)
                .expect("brush model parses");
            let polys_payload = archive
                .export_payload(model.polys_ref as usize - 1)
                .unwrap();
            let raw_polys = parse_upolys(polys_payload, archive).unwrap();
            total_polys += raw_polys.len();
            for raw in &raw_polys {
                let world_verts: Vec<[f32; 3]> = raw
                    .vertices
                    .iter()
                    .map(|v| hand_transform(location, rotation, scale, *v))
                    .collect();
                let all_found = world_verts.iter().all(|wv| {
                    scene
                        .polys
                        .iter()
                        .any(|sp| sp.vertices.iter().any(|sv| close(*sv, *wv, 0.5)))
                });
                if all_found {
                    matched_polys += 1;
                }
            }
            checked += 1;
        }
        assert!(saw_423, "Brush423 checked");
        assert!(checked >= 4, "four ordinary brushes checked, got {checked}");
        assert!(
            matched_polys >= total_polys * 9 / 10,
            "hand-computed polys matched {matched_polys}/{total_polys}"
        );
        println!(
            "[g4] brush transforms: {checked} brushes, {matched_polys}/{total_polys} polys matched hand-computed world vertices"
        );
    }

    /// (2) Camera: the scene camera is the PlayerStart placement, exactly.
    #[test]
    fn camera_is_playerstart_placement() {
        let Loaded {
            world,
            level,
            scene,
        } = load("PrivetDr");
        let arena = &world.arena;
        let mut expected = None;
        for &actor in &level.actors {
            if class_label(arena, actor).eq_ignore_ascii_case("PlayerStart") {
                let store = arena.get(actor).unwrap().properties().unwrap();
                expected = Some((
                    struct_vec3(store, arena, "Location").unwrap_or([0.0; 3]),
                    struct_ints(store, arena, "Rotation").unwrap_or([0; 3]),
                ));
                break;
            }
        }
        let (location, rotation) = expected.expect("PlayerStart present");
        let cam = scene.camera.expect("camera resolved");
        assert!(
            close(cam.location, location, 0.01),
            "camera {:?} != PlayerStart {:?}",
            cam.location,
            location
        );
        assert_eq!(
            cam.rotation, rotation,
            "camera rotation != PlayerStart rotation"
        );
        println!(
            "[g4] camera: PlayerStart Location={location:?} Rotation={rotation:?} == scene camera {:?}/{:?}",
            cam.location, cam.rotation
        );
    }

    /// (3) Coverage: every Model export is either brush-referenced or a
    /// level model, and the scene poly total equals the sum of per-model
    /// decoded poly counts.
    #[test]
    fn coverage_all_models_present() {
        let Loaded {
            world,
            level,
            scene,
        } = load("PrivetDr");
        let arena = &world.arena;
        let archive = level.archive.as_ref().unwrap();

        let mut referenced: std::collections::HashSet<usize> = Default::default();
        for &actor in &level.actors {
            if !class_label(arena, actor).eq_ignore_ascii_case("Brush") {
                continue;
            }
            let Some(store) = arena.get(actor).unwrap().properties() else {
                continue;
            };
            if let Some(PropValue::Object(Some(raw))) =
                arena.names.find_index("Brush").and_then(|n| store.get(n))
                && *raw > 0
            {
                referenced.insert(*raw as usize - 1);
            }
        }

        let mut model_exports = 0usize;
        let mut expected_polys = 0usize;
        for index in 0..archive.exports.len() {
            let entry = &archive.exports[index];
            if ref_class_text(archive, entry.class_ref) != Some("Model") {
                continue;
            }
            model_exports += 1;
            let Some(payload) = archive.export_payload(index) else {
                continue;
            };
            let Ok(model) = parse_model(payload, archive, entry.class_ref) else {
                continue;
            };
            if !model.nodes.is_empty() {
                // Level model: one scene poly per renderable node.
                expected_polys += model
                    .nodes
                    .iter()
                    .filter(|n| {
                        n.vertex_count >= 3
                            && n.surface >= 0
                            && (n.surface as usize) < model.surfs.len()
                            && n.vertex_pool + n.vertex_count as usize <= model.verts.len()
                    })
                    .count();
            } else if model.polys_ref > 0
                && let Some(polys_payload) =
                    archive.export_payload(model.polys_ref as usize - 1)
                && let Ok(raw) = parse_upolys(polys_payload, archive)
            {
                expected_polys += raw.len();
            }
        }

        let unreferenced = model_exports - referenced.len();
        assert!(
            unreferenced <= 8,
            "unexpected orphan models: {unreferenced} of {model_exports}"
        );
        assert_eq!(
            scene.polys.len(),
            expected_polys,
            "scene poly total != sum of per-model decoded counts"
        );
        println!(
            "[g4] coverage: {model_exports} model exports, {} brush-referenced, {unreferenced} level/orphan, scene polys={} == decoded sum {expected_polys}",
            referenced.len(),
            scene.polys.len()
        );
    }

    /// (4) Winding: each ScenePoly's stored normal agrees with the vertex
    /// order (cross(v1-v0, v2-v0) · normal > 0).
    #[test]
    fn winding_matches_normal_everywhere() {
        let Loaded {
            world,
            level,
            scene,
        } = load("PrivetDr");
        let _ = (&world, &level);
        let mut agree = 0usize;
        for poly in &scene.polys {
            if poly.vertices.len() < 3 {
                continue;
            }
            // Newell's method: robust for near-collinear leading vertices
            // (fan-triangulated records often start with collinear edges).
            let mut n = [0.0f32; 3];
            for i in 0..poly.vertices.len() {
                let a = poly.vertices[i];
                let b = poly.vertices[(i + 1) % poly.vertices.len()];
                n[0] += (a[1] - b[1]) * (a[2] + b[2]);
                n[1] += (a[2] - b[2]) * (a[0] + b[0]);
                n[2] += (a[0] - b[0]) * (a[1] + b[1]);
            }
            let mag2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
            if mag2 < 1.0 {
                // Degenerate (zero-area) records carry no winding contract.
                continue;
            }
            let d = n[0] * poly.normal[0] + n[1] * poly.normal[1] + n[2] * poly.normal[2];
            if d <= 0.0 {
                panic!(
                    "winding disagrees: normal={:?} newell={:?} verts={:?} flags={:08x}",
                    poly.normal, n, poly.vertices, poly.poly_flags
                );
            }
            agree += 1;
        }
        println!("[g4] winding: {agree} polys cross==normal, 0 disagree");
    }

    /// (5) Textures: every poly's texture ref resolves or is loudly
    /// counted; UVs on one known poly stay in a sane texel range.
    #[test]
    fn textures_resolve_and_uv_sane() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../HarryPotter2/Unreal");
        let Loaded {
            world,
            level,
            scene,
        } = load("PrivetDr");
        let arena = &world.arena;
        let none_count = scene.polys.iter().filter(|p| p.texture.is_none()).count();
        let mut store = crate::utx::TextureStore::new(&root);
        let mut resolved = 0usize;
        let mut loud: std::collections::HashMap<String, usize> = Default::default();
        for key in scene.polys.iter().filter_map(|p| p.texture.as_ref()) {
            if store.resolve(key).is_ok() {
                resolved += 1;
            } else {
                *loud.entry(key.package.clone()).or_insert(0) += 1;
            }
        }
        assert!(none_count <= 8, "{none_count} polys carry no texture ref");
        assert!(
            resolved > scene.polys.len() * 9 / 10,
            "only {resolved}/{} textures resolve",
            scene.polys.len()
        );
        println!(
            "[g4] textures: {resolved}/{} resolved, loud fallbacks {loud:?}, no-ref {none_count}",
            scene.polys.len()
        );

        // UV sanity on Brush423's first poly: dot(P-Base, TexU/V)/size - pan
        // must be finite and bounded for a real wall texture.
        let archive = level.archive.as_ref().unwrap();
        let mut uv_max = 0.0f32;
        let mut sampled = 0usize;
        'outer: for &actor in &level.actors {
            if !class_label(arena, actor).eq_ignore_ascii_case("Brush") {
                continue;
            }
            if arena.display_name(actor).unwrap_or_default() != "Brush423" {
                continue;
            }
            let store = arena.get(actor).unwrap().properties().unwrap();
            let raw = match arena.names.find_index("Brush").and_then(|n| store.get(n)) {
                Some(PropValue::Object(Some(raw))) => *raw,
                _other => {
                    break;
                }
            };
            let payload = archive.export_payload(raw as usize - 1).unwrap();
            let model = parse_model(
                payload,
                archive,
                archive.exports[raw as usize - 1].class_ref,
            )
            .unwrap();
            let polys_payload = archive
                .export_payload(model.polys_ref as usize - 1)
                .unwrap();
            let parsed = parse_upolys(polys_payload, archive).unwrap();
            for raw_poly in &parsed {
                let size = 256.0f32; // mip-0 wall texture size class
                for v in raw_poly.vertices.iter().take(2) {
                    let d = [
                        v[0] - raw_poly.base[0],
                        v[1] - raw_poly.base[1],
                        v[2] - raw_poly.base[2],
                    ];
                    let u = (d[0] * raw_poly.tex_u[0]
                        + d[1] * raw_poly.tex_u[1]
                        + d[2] * raw_poly.tex_u[2]
                        - raw_poly.pan_uv[0])
                        / size;
                    let w = (d[0] * raw_poly.tex_v[0]
                        + d[1] * raw_poly.tex_v[1]
                        + d[2] * raw_poly.tex_v[2]
                        - raw_poly.pan_uv[1])
                        / size;
                    assert!(u.is_finite() && w.is_finite(), "UV not finite");
                    uv_max = uv_max.max(u.abs()).max(w.abs());
                    sampled += 1;
                }
                if sampled >= 2 {
                    break 'outer;
                }
            }
        }
        assert!(sampled >= 2, "sampled a known poly's UVs");
        assert!(uv_max < 64.0, "UV magnitude {uv_max} out of sane range");
        println!("[g4] UVs: Brush423 first-poly |uv|max={uv_max} (sampled {sampled})");
    }
}
