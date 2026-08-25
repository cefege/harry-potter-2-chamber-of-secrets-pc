//! Texture package loading: resolves [`TextureKey`]s against `.utx` packages
//! under `<data_root>/Textures` and decodes their mips (P8 palette-indexed or
//! DXT1) into upload-ready surfaces.
//!
//! Wire ground truth: `UnTex.cpp` `SerializeMips` / `FMipmap::operator<<`,
//! `UPalette::Serialize`; `FMipmap` = `TLazyArray<BYTE>` (i32 skip-pos, then
//! AR_INDEX num, inline pixel bytes) followed by `USize,VSize` (i32 each) and
//! `UBits,VBits` (u8 each). Verified against every shipped `.utx`: the count
//! is a compact index and no `Max` field exists on the wire.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use hp_format::package79::{
    ByteCursor, NameEntry, PackageArchive, PropertyTag, read_compact_index, read_property_tags,
};

use crate::error::{EngineError, Result};
use crate::scene::TextureKey;

/// `ETextureFormat::TEXF_P8` — palette-indexed bytes.
const TEXF_P8: u8 = 0;
/// `ETextureFormat::TEXF_DXT1` — BC1 blocks.
const TEXF_DXT1: u8 = 3;

/// One decoded texture surface.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedFormat {
    /// Ready-to-upload BGRA8 pixels (`width * height * 4` bytes).
    Bgra8(Vec<u8>),
    /// P8 indices plus the 256-entry RGBA palette (1024 bytes).
    Indexed8 {
        indices: Vec<u8>,
        palette: Box<[u8; 256 * 4]>,
    },
}

#[derive(Debug, Clone)]
pub struct DecodedTexture {
    pub format: DecodedFormat,
    pub width: u32,
    pub height: u32,
}

/// Lazily caches opened `.utx` archives per package stem.
#[derive(Default)]
pub struct TextureStore {
    data_root: PathBuf,
    archives: HashMap<String, Option<PackageArchive>>,
}

impl TextureStore {
    pub fn new(data_root: &Path) -> TextureStore {
        TextureStore {
            data_root: data_root.to_path_buf(),
            archives: HashMap::new(),
        }
    }

    /// Resolve one key to a decoded base mip. Missing package/object or an
    /// unsupported pixel format is a loud reason-coded failure
    /// (`renderer.texture_*`).
    pub fn resolve(&mut self, key: &TextureKey) -> Result<DecodedTexture> {
        let archive = self.archive_for(&key.package)?;
        let index = find_export(archive, &key.object_path)?;
        decode_texture(archive, index)
    }

    /// Open (and memoize) the `.utx` whose file stem matches `package`
    /// case-insensitively; UE1 package references ignore case.
    fn archive_for(&mut self, package: &str) -> Result<&PackageArchive> {
        let stem = package.to_ascii_lowercase();
        if !self.archives.contains_key(&stem) {
            let archive = load_archive(&self.data_root, package)?;
            self.archives.insert(stem.clone(), Some(archive));
        }
        Ok(self
            .archives
            .get(&stem)
            .and_then(|slot| slot.as_ref())
            .expect("slot populated above"))
    }
}

/// Locate `<root>/Textures/<package>.utx` and parse it.
fn load_archive(data_root: &Path, package: &str) -> Result<PackageArchive> {
    let dir = data_root.join("Textures");
    let mut found = None;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let matches = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("utx"))
                && path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case(package));
            if matches {
                found = Some(path);
                break;
            }
        }
    }
    let Some(path) = found else {
        return Err(EngineError::new(
            "renderer.texture_package_missing",
            format!("no Textures/{package}.utx under {}", data_root.display()),
        ));
    };
    let bytes = std::fs::read(&path).map_err(|err| {
        EngineError::new(
            "renderer.texture_read",
            format!("{}: {err}", path.display()),
        )
    })?;
    hp_format::package79::read_package(&bytes)
        .map_err(|err| texture_parse(format!("{}: {err}", path.display())))
}

/// Walk a dotted object path through the export table via outer links
/// (top-level exports carry `outer_ref == 0`); every step compares
/// case-insensitively.
fn find_export(archive: &PackageArchive, object_path: &str) -> Result<usize> {
    let missing = || {
        EngineError::new(
            "renderer.texture_object_missing",
            format!("object `{object_path}` not found"),
        )
    };

    let mut scope: Vec<usize> = (0..archive.exports.len())
        .filter(|&i| archive.exports[i].outer_ref == 0)
        .collect();
    let mut current = None;
    for segment in object_path.split('.') {
        let matched = scope
            .iter()
            .copied()
            .find(|&i| export_name(archive, i).is_some_and(|n| n.eq_ignore_ascii_case(segment)))
            .ok_or_else(missing)?;
        scope = (0..archive.exports.len())
            .filter(|&j| archive.exports[j].outer_ref as usize == matched + 1)
            .collect();
        current = Some(matched);
    }
    current.ok_or_else(missing)
}

/// Parse one texture export: tagged properties, then the base-mip binary
/// section (`SerializeMips`). Any `bHasComp` compressed-mip block that may
/// follow is deliberately left unread.
fn decode_texture(archive: &PackageArchive, index: usize) -> Result<DecodedTexture> {
    let entry = archive.exports[index];
    archive
        .export_payload(index)
        .ok_or_else(|| texture_parse(format!("texture export {index} has no payload")))?;
    // serial_offset is file-absolute; the payload region starts at names_end.
    let start = entry.serial_offset.unwrap_or(0) as usize - archive.names_end();
    let region_base = archive.names_end();
    let mut cur = ByteCursor::at(archive.payload_bytes(), start);

    let properties = read_property_tags(&mut cur, &archive.names)
        .map_err(|err| texture_parse(err.to_string()))?;
    let format = property_by_name(&properties, &archive.names, "Format")
        .and_then(|tag| tag.payload.first().copied())
        .unwrap_or(TEXF_P8);
    // Object references serialize as compact indices (payload size 1 byte
    // for the common in-package refs).
    let palette_ref = property_by_name(&properties, &archive.names, "Palette").and_then(|tag| {
        let mut cur = ByteCursor::new(&tag.payload);
        read_compact_index(&mut cur).ok()
    });

    // First SerializeMips block: base mips. Only element 0 (the largest) is
    // decoded; its lazy-array skip position hands back the stream for the
    // trailing dimension fields.
    let mip_count = read_compact_index(&mut cur).map_err(|err| texture_parse(err.to_string()))?;
    if mip_count <= 0 {
        return Err(texture_parse("texture declares no mips"));
    }
    let mip = read_base_mip(&mut cur, region_base)?;

    match format {
        TEXF_P8 => {
            let expected = mip.width as usize * mip.height as usize;
            if mip.data.len() != expected {
                return Err(texture_parse(format!(
                    "P8 index buffer holds {} bytes, expected {expected}",
                    mip.data.len()
                )));
            }
            let palette = resolve_palette(archive, palette_ref)?;
            Ok(DecodedTexture {
                format: DecodedFormat::Indexed8 {
                    indices: mip.data,
                    palette,
                },
                width: mip.width,
                height: mip.height,
            })
        }
        TEXF_DXT1 => {
            let pixels = hp_format::dxt1::decode_dxt1_to_bgra(&mip.data, mip.width, mip.height)
                .map_err(|err| EngineError::new("renderer.texture_decode", err.to_string()))?;
            Ok(DecodedTexture {
                format: DecodedFormat::Bgra8(pixels),
                width: mip.width,
                height: mip.height,
            })
        }
        other => Err(EngineError::new(
            "renderer.texture_format_unsupported",
            format!("ETextureFormat {other} has no runtime decoder"),
        )),
    }
}

/// One `FMipmap` element: `TLazyArray<BYTE>` header + inline pixels, resumed
/// at the saved skip position, then `USize,VSize,UBits,VBits`.
fn read_base_mip(cur: &mut ByteCursor<'_>, region_base: usize) -> Result<BaseMip> {
    let parse = |err: hp_format::package79::PackageError| texture_parse(err.to_string());

    let seek_pos = cur.i32().map_err(parse)?;
    let num = read_compact_index(cur).map_err(parse)?;
    if num < 0 {
        return Err(texture_parse("mip declares a negative byte count"));
    }
    let data_start = cur.position();
    let data = cur.take(num as usize).map_err(parse)?.to_vec();

    // The lazy-array skip position is file-absolute and lands just past the
    // inline pixel bytes; resume there so later fields stay aligned. Empty
    // placeholder mips (num == 0, seen in Palettes.utx) save a skip position
    // with no inline data at all — leave the cursor in place and let the
    // dimension check reject them.
    if seek_pos >= 0 && num > 0 {
        let rel = seek_pos as usize;
        if rel < region_base + data_start || rel > region_base + cur.len() {
            return Err(texture_parse("lazy-array skip position out of range"));
        }
        cur.seek(rel - region_base);
    }

    let width = cur.i32().map_err(parse)?;
    let height = cur.i32().map_err(parse)?;
    let _ubits = cur.u8().map_err(parse)?;
    let _vbits = cur.u8().map_err(parse)?;
    if width <= 0 || height <= 0 {
        return Err(texture_parse("mip declares a zero-sized surface"));
    }
    Ok(BaseMip {
        data,
        width: width as u32,
        height: height as u32,
    })
}

struct BaseMip {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

/// Decode a `UPalette` export payload: tagged properties, then
/// `TArray<FColor>` of 256 B,G,R,A quads, reordered to RGBA.
fn resolve_palette(archive: &PackageArchive, palette_ref: Option<i32>) -> Result<Box<[u8; 1024]>> {
    let missing = || {
        EngineError::new(
            "renderer.texture_palette_missing",
            "P8 texture has no usable Palette reference",
        )
    };
    let Some(reference) = palette_ref.filter(|r| *r != 0) else {
        return Err(missing());
    };
    if reference < 0 {
        return Err(EngineError::new(
            "renderer.texture_cross_package",
            format!("palette reference {reference} targets another package"),
        ));
    }

    let index = reference as usize - 1;
    let payload = archive
        .export_payload(index)
        .ok_or_else(|| texture_parse(format!("palette export {index} has no payload")))?;
    let mut cur = ByteCursor::new(payload);
    read_property_tags(&mut cur, &archive.names).map_err(|err| texture_parse(err.to_string()))?;

    let parse = |err: hp_format::package79::PackageError| texture_parse(err.to_string());
    let num = read_compact_index(&mut cur).map_err(parse)?;
    if num != 256 {
        return Err(texture_parse(format!(
            "palette export {index} holds {num} colors, expected 256"
        )));
    }
    let raw = cur.take(256 * 4).map_err(parse)?;
    let mut palette = Box::new([0u8; 1024]);
    for (slot, quad) in palette.chunks_exact_mut(4).zip(raw.chunks_exact(4)) {
        // Stored B,G,R,A → RGBA.
        slot[0] = quad[2];
        slot[1] = quad[1];
        slot[2] = quad[0];
        slot[3] = quad[3];
    }
    Ok(palette)
}

fn property_by_name<'a>(
    properties: &'a [PropertyTag],
    names: &[NameEntry],
    want: &str,
) -> Option<&'a PropertyTag> {
    properties.iter().find(|tag| {
        names
            .get(tag.name_index.max(0) as usize)
            .is_some_and(|n| n.text.eq_ignore_ascii_case(want))
    })
}

fn export_name(archive: &PackageArchive, index: usize) -> Option<&str> {
    let entry = archive.exports.get(index)?;
    if entry.object_name_index < 0 {
        return None;
    }
    archive
        .names
        .get(entry.object_name_index as usize)
        .map(|n| n.text.as_str())
}

fn texture_parse(detail: impl Into<String>) -> EngineError {
    EngineError::new("renderer.texture_parse", detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp_format::package79::read_package;

    /// Repo root: `crates/hp-engine` sits two levels below it.
    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("manifest dir sits two levels below the repo root")
            .to_path_buf()
    }

    fn textures_dir() -> Option<PathBuf> {
        data_root().map(|root| root.join("Textures"))
    }

    /// Data root whose `Textures/` subtree backs [`textures_dir`].
    /// Game data root: `HP2_DATA_ROOT` overrides, else the repo-relative
    /// retail tree; `None` when the retail `System/Default.ini` is absent
    /// (fresh clones ship without the gitignored game content).
    fn data_root() -> Option<PathBuf> {
        let root = std::env::var("HP2_DATA_ROOT")
            .map(PathBuf::from)
            .ok()
            .unwrap_or_else(|| repo_root().join("HarryPotter2").join("Unreal"));
        (root.join("System/Default.ini").is_file()).then_some(root)
    }

    /// Standard blocked-notice for data-dependent tests.
    fn blocked() {
        println!("blocked: game data root absent");
    }

    fn class_name(archive: &PackageArchive, class_ref: i32) -> Option<&str> {
        let name_index = if class_ref > 0 {
            archive
                .exports
                .get(class_ref as usize - 1)?
                .object_name_index
        } else {
            archive
                .imports
                .get((-class_ref as usize) - 1)?
                .object_name_index
        };
        if name_index < 0 {
            return None;
        }
        archive
            .names
            .get(name_index as usize)
            .map(|n| n.text.as_str())
    }

    /// Every export whose class is `Engine.Texture`.
    fn enumerate_textures(archive: &PackageArchive) -> Vec<usize> {
        archive
            .exports
            .iter()
            .enumerate()
            .filter(|(_, entry)| class_name(archive, entry.class_ref) == Some("Texture"))
            .map(|(index, _)| index)
            .collect()
    }

    /// Dotted path of an export, rebuilt from the outer chain.
    fn object_path(archive: &PackageArchive, index: usize) -> String {
        let mut parts = vec![
            export_name(archive, index)
                .expect("named export")
                .to_string(),
        ];
        let mut outer = archive.exports[index].outer_ref;
        while outer > 0 {
            let parent = outer as usize - 1;
            parts.push(
                export_name(archive, parent)
                    .expect("named outer")
                    .to_string(),
            );
            outer = archive.exports[parent].outer_ref;
        }
        parts.reverse();
        parts.join(".")
    }

    /// `Format` property value of a texture export (default P8 when absent).
    fn stored_format(archive: &PackageArchive, index: usize) -> Option<u8> {
        let payload = archive.export_payload(index)?;
        let mut cur = ByteCursor::at(payload, 0);
        let properties = read_property_tags(&mut cur, &archive.names).ok()?;
        Some(
            property_by_name(&properties, &archive.names, "Format")
                .and_then(|tag| tag.payload.first().copied())
                .unwrap_or(TEXF_P8),
        )
    }

    fn is_pow2_at_least_4(value: u32) -> bool {
        value >= 4 && value.is_power_of_two()
    }

    #[test]
    fn master_package_parses_with_many_exports() {
        let Some(textures_dir) = textures_dir() else {
            blocked();
            return;
        };
        let bytes = std::fs::read(textures_dir.join("HP2_Master.utx")).expect("master utx");
        let archive = read_package(&bytes).expect("parse");
        assert!(
            archive.exports.len() > 50,
            "{} exports",
            archive.exports.len()
        );
    }

    /// Scan the shipped packages for a real texture and decode it as P8.
    /// No shipped `.utx` serializes a non-default `Format` property — every
    /// base mip is P8 by class default — so DXT1 coverage lives in
    /// [`dxt1_key_decodes_bgra8`], which hand-assembles a minimal package.
    #[test]
    fn real_package_key_decodes_p8() {
        let Some(textures_dir) = textures_dir() else {
            blocked();
            return;
        };
        let mut p8 = None;
        for entry in std::fs::read_dir(&textures_dir).expect("textures dir") {
            let path = entry.expect("entry").path();
            if !path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("utx"))
            {
                continue;
            }
            if p8.is_some() {
                break;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(archive) = read_package(&bytes) else {
                continue;
            };
            let package = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("utx stem")
                .to_string();
            for index in enumerate_textures(&archive) {
                if stored_format(&archive, index) == Some(TEXF_P8) {
                    p8 = Some(TextureKey {
                        package: package.clone(),
                        object_path: object_path(&archive, index),
                    });
                    break;
                }
            }
        }
        let p8 = p8.expect("some P8 texture across the shipped packages");

        let Some(root) = data_root() else {
            blocked();
            return;
        };
        let mut store = TextureStore::new(&root);
        let decoded = store.resolve(&p8).expect("P8 decode");
        assert!(is_pow2_at_least_4(decoded.width));
        assert!(is_pow2_at_least_4(decoded.height));
        let DecodedFormat::Indexed8 { indices, palette } = decoded.format else {
            panic!("P8 key decoded as {:?}", decoded.format);
        };
        assert_eq!(indices.len(), (decoded.width * decoded.height) as usize);
        assert_eq!(palette.len(), 1024);
    }

    #[test]
    fn dxt1_key_decodes_bgra8() {
        use hp_format::package79::{PACKAGE_TAG, write_compact_index};

        const EDGE: u32 = 8;
        // One BC1 block per 4x4 tile; c0 = red 565, c1 = blue, indices 0.
        let mut block = Vec::new();
        block.extend_from_slice(&0xF800u16.to_le_bytes());
        block.extend_from_slice(&0x001Fu16.to_le_bytes());
        block.extend_from_slice(&0u32.to_le_bytes());
        let blocks = vec![block; hp_format::dxt1::encoded_size(EDGE, EDGE) / 8].concat();

        // Tagged properties: Format (ByteProperty kind 1) = TEXF_DXT1, then
        // the "None" terminator.
        let mut props = Vec::new();
        write_compact_index(&mut props, 1); // name "Format"
        props.push(0x71); // Info byte: kind 1 | size code 0x70 (i32 size)
        props.extend_from_slice(&1i32.to_le_bytes());
        props.push(TEXF_DXT1);
        write_compact_index(&mut props, 0); // "None"

        let mut num_bytes = Vec::new();
        write_compact_index(&mut num_bytes, blocks.len() as i32);

        // Mip section inside the payload: compact mip count, lazy-array
        // element (skip-pos placeholder patched once offsets are known),
        // then USize/VSize/UBits/VBits.
        let mut mips = Vec::new();
        write_compact_index(&mut mips, 1);
        let seek_slot = mips.len();
        mips.extend_from_slice(&0i32.to_le_bytes());
        mips.extend_from_slice(&num_bytes);
        mips.extend_from_slice(&blocks);
        mips.extend_from_slice(&(EDGE as i32).to_le_bytes());
        mips.extend_from_slice(&(EDGE as i32).to_le_bytes());
        mips.push(3); // UBits
        mips.push(3); // VBits
        let mut payload = props.clone();
        payload.extend_from_slice(&mips);

        // Names: None, Format, DxtTex — compact unit count + latin1 bytes +
        // NUL + flags u32 each. The summary header for v79 is 64 bytes.
        let mut names_region = Vec::new();
        for text in ["None", "Format", "DxtTex"] {
            write_compact_index(&mut names_region, text.len() as i32 + 1);
            names_region.extend_from_slice(text.as_bytes());
            names_region.push(0);
            names_region.extend_from_slice(&0u32.to_le_bytes());
        }
        const NAME_OFFSET: usize = 64;
        let names_end = NAME_OFFSET + names_region.len();

        // Skip position lands just past the inline pixel bytes.
        let blocks_start = props.len() + 1 + 4 + num_bytes.len();
        let seek_abs = (names_end + blocks_start + blocks.len()) as i32;
        payload[props.len() + seek_slot..props.len() + seek_slot + 4]
            .copy_from_slice(&seek_abs.to_le_bytes());

        let import_offset = names_end + payload.len();
        let export_offset = import_offset; // no imports

        // One top-level export named "DxtTex".
        let mut exports_region = Vec::new();
        write_compact_index(&mut exports_region, 0); // class_ref
        write_compact_index(&mut exports_region, 0); // super_ref
        exports_region.extend_from_slice(&0i32.to_le_bytes()); // outer_ref
        write_compact_index(&mut exports_region, 2); // object_name_index
        exports_region.extend_from_slice(&0u32.to_le_bytes()); // object_flags
        write_compact_index(&mut exports_region, payload.len() as i32);
        write_compact_index(&mut exports_region, names_end as i32);

        let mut out = Vec::new();
        out.extend_from_slice(&PACKAGE_TAG.to_le_bytes());
        out.extend_from_slice(&79u32.to_le_bytes()); // version_word
        out.extend_from_slice(&0u32.to_le_bytes()); // package flags
        out.extend_from_slice(&3i32.to_le_bytes()); // name count
        out.extend_from_slice(&(NAME_OFFSET as i32).to_le_bytes());
        out.extend_from_slice(&1i32.to_le_bytes()); // export count
        out.extend_from_slice(&(export_offset as i32).to_le_bytes());
        out.extend_from_slice(&0i32.to_le_bytes()); // import count
        out.extend_from_slice(&(import_offset as i32).to_le_bytes());
        out.extend_from_slice(&[0u8; 16]); // guid
        out.extend_from_slice(&1i32.to_le_bytes()); // generation count
        out.extend_from_slice(&1i32.to_le_bytes()); // exports in generation
        out.extend_from_slice(&3i32.to_le_bytes()); // names in generation
        assert_eq!(out.len(), NAME_OFFSET);
        out.extend_from_slice(&names_region);
        out.extend_from_slice(&payload);
        out.extend_from_slice(&exports_region);

        let root = std::env::temp_dir().join("hp2_utx_dxt1_test");
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("Textures")).expect("mkdir");
        std::fs::write(root.join("Textures").join("SynthDxt.utx"), &out).expect("write utx");

        let mut store = TextureStore::new(&root);
        let decoded = store
            .resolve(&TextureKey {
                package: "synthdxt".to_string(),
                object_path: "DxtTex".to_string(),
            })
            .expect("decode synthetic DXT1 texture");
        assert_eq!((decoded.width, decoded.height), (EDGE, EDGE));
        match decoded.format {
            DecodedFormat::Bgra8(pixels) => {
                assert_eq!(pixels.len(), (EDGE * EDGE * 4) as usize);
                // Index 0 of every block selects c0 = red.
                assert_eq!(&pixels[..4], &[0, 0, 255, 255]);
            }
            other => panic!("expected Bgra8, got {other:?}"),
        }
    }

    #[test]
    fn unknown_package_fails_loud() {
        let Some(root) = data_root() else {
            blocked();
            return;
        };
        let mut store = TextureStore::new(&root);
        let key = TextureKey {
            package: "DefinitelyNotAShippedPackage".to_string(),
            object_path: "anything".to_string(),
        };
        let err = store.resolve(&key).expect_err("unknown package");
        assert!(
            err.reason_code.starts_with("renderer.texture_"),
            "got {}",
            err.reason_code
        );
    }
}
