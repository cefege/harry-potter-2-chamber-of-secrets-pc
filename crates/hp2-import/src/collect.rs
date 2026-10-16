//! Replaces the shared `Asset`/`MakeAsset` from `HP2ImportInternal.h` and
//! `HP2ImportCollect.cpp`'s `Insert`/`Posix`/`SourceAsset`/`CollectSide`:
//! turning a source tree (prototype or retail) into the set of overlay
//! assets it contributes.
//!
//! [`collect_side`] verified against the real compiled `CollectSide` --
//! not a hand-transcribed probe: `c++ main.cpp
//! HarryPotter2/Unreal/Import/Src/HP2ImportCollect.cpp` (plus the pinned
//! `utf8proc` and CommonCrypto), calling
//! `HP2Import::Detail::CollectSide` directly on a fixture exercising
//! every `collectTo` category, the Help-is-optional path, and the
//! System/-prohibited-file eager-Fail loop. Exact match against
//! [`collect_side`] on the identical fixture: same paths, same SHA-256
//! hashes, same sizes, same error text on the prohibited-file case.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::classify::{is_debris, is_prohibited};
use crate::files::{child, child_file, walk};
use crate::hash::hash;
use crate::lower::lower;

fn fail(message: impl Into<String>) -> String {
    message.into()
}

/// Replaces `fs::path::lexically_relative`, purely by comparing path
/// components -- no filesystem access, matching the C++ function. Rust's
/// `Path::strip_prefix` is not a substitute: it returns `Err` (not an
/// empty path) for a path outside `base`, and never produces a `../`
/// prefix, so naively falling back to the original path on its `Err`
/// would silently accept an escaping path with the absolute path left
/// in `source_path` instead of erroring -- exactly the bug this
/// function exists to avoid repeating.
///
/// `None` matches C++ returning an empty path: `base`/`path` disagree on
/// absolute-vs-relative (the only mismatch this project's inputs can hit
/// in practice; both come from directory walks under an absolute root).
/// Otherwise returns `base`'s number of non-shared trailing components
/// worth of `..`, followed by `path`'s own non-shared trailing
/// components -- e.g. `/a/b/c` relative to `/a/x/y` is `../../b/c`.
fn lexically_relative(base: &Path, path: &Path) -> Option<PathBuf> {
    if base.is_absolute() != path.is_absolute() {
        return None;
    }
    let base_components: Vec<_> = base.components().collect();
    let path_components: Vec<_> = path.components().collect();
    let common = base_components
        .iter()
        .zip(path_components.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut result = PathBuf::new();
    for _ in 0..(base_components.len() - common) {
        result.push("..");
    }
    for component in &path_components[common..] {
        result.push(component.as_os_str());
    }
    Some(result)
}

fn utf8(p: &Path) -> Result<String, String> {
    p.to_str()
        .map(str::to_string)
        .ok_or_else(|| fail(format!("non-UTF-8 path: {}", p.to_string_lossy())))
}

/// Replaces `Posix`: backslash-to-forward-slash, nothing else (not a full
/// path normalization -- callers already work with POSIX-relative
/// fragments by the time this runs).
fn posix(path: &str) -> String {
    path.replace('\\', "/")
}

/// Mirrors `HP2ImportInternal.h`'s `Asset` struct field for field.
/// `classification`, `link`, `content`, `generated`, and `sources` are
/// left at their C++ default values here ([`make_asset`] never sets
/// them), matching `Asset a; a.source = ...` in C++ leaving the rest
/// default-constructed -- later pipeline stages (not yet ported) fill
/// them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub source: String,
    pub source_path: String,
    pub path: String,
    pub classification: String,
    pub hash: String,
    pub link: String,
    pub disk: PathBuf,
    pub content: String,
    pub generated: bool,
    pub size: u64,
    pub sources: Vec<String>,
}

impl Default for Asset {
    fn default() -> Self {
        Asset {
            source: String::new(),
            source_path: String::new(),
            path: String::new(),
            classification: String::new(),
            hash: String::new(),
            link: String::new(),
            disk: PathBuf::new(),
            content: String::new(),
            generated: false,
            size: 0,
            sources: Vec::new(),
        }
    }
}

/// Replaces `MakeAsset`. `file` must be a real (non-symlink) regular
/// file under `root`, and `dest` must not be [`is_prohibited`]. The
/// root-escape check (via [`lexically_relative`]) was verified against
/// the real compiled `MakeAsset` on both an escaping path (two sibling
/// absolute directories, `file` under the one that isn't `root`) and a
/// valid one: identical "input asset escapes ... root: ..." text and
/// identical success `sourcePath` on the matching inputs.
pub fn make_asset(source: &str, root: &Path, file: &Path, dest: String) -> Result<Asset, String> {
    let is_symlink = fs::symlink_metadata(file)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);
    let is_regular = fs::metadata(file).map(|m| m.is_file()).unwrap_or(false);
    if is_symlink || !is_regular {
        return Err(fail(format!(
            "input asset is not a real regular file: {}",
            utf8(file)?
        )));
    }
    let rel = lexically_relative(root, file).unwrap_or_default();
    let rel_str = utf8(&rel)?;
    if rel_str.is_empty() || rel_str.starts_with("../") {
        return Err(fail(format!(
            "input asset escapes {source} root: {}",
            utf8(file)?
        )));
    }
    if is_prohibited(&dest) {
        return Err(fail(format!(
            "prohibited asset selected for overlay: {dest}"
        )));
    }
    let (size, digest) = hash(file)?;
    Ok(Asset {
        source: source.to_string(),
        source_path: rel_str,
        path: dest,
        hash: digest,
        disk: file.to_path_buf(),
        size,
        ..Asset::default()
    })
}

/// Replaces `SourceAsset`.
fn source_asset(
    source: &str,
    root: &Path,
    file: &Path,
    destination: String,
) -> Result<Asset, String> {
    let mut asset = make_asset(source, root, file, destination)?;
    asset.source_path = posix(&asset.source_path);
    Ok(asset)
}

/// Replaces `Insert`. Keyed by `Lower(asset.path)`, matching C++'s
/// `std::map<std::string, Asset>` -- a `BTreeMap` iterates in the same
/// sorted-by-key order a `std::map<std::string, _>` does (both order by
/// byte value of the key).
fn insert(assets: &mut BTreeMap<String, Asset>, asset: Asset) -> Result<(), String> {
    let key = lower(&asset.path);
    if let Some(existing) = assets.get(&key) {
        return Err(fail(format!(
            "case-insensitive output collision for {}: {} and {}",
            asset.path, existing.source_path, asset.source_path
        )));
    }
    assets.insert(key, asset);
    Ok(())
}

/// Replaces `CollectSide`'s `collectTo` lambda: walk `base`, keep entries
/// `accept` returns true for, and insert each as a `destination_root`
/// relative asset.
fn collect_to(
    assets: &mut BTreeMap<String, Asset>,
    root: &Path,
    source: &str,
    base: &Path,
    destination_root: &str,
    recursive: bool,
    accept: impl Fn(&Path) -> bool,
) -> Result<(), String> {
    for file in walk(base, recursive)? {
        if !accept(&file) {
            continue;
        }
        let rel = file.strip_prefix(base).unwrap_or(&file);
        let destination = format!("{destination_root}/{}", posix(&utf8(rel)?));
        let asset = source_asset(source, root, &file, destination)?;
        insert(assets, asset)?;
    }
    Ok(())
}

fn lower_extension(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => lower(&format!(".{ext}")),
        None => String::new(),
    }
}

/// Replaces `CollectSide`.
pub fn collect_side(root: &Path, source: &str) -> Result<BTreeMap<String, Asset>, String> {
    let mut assets: BTreeMap<String, Asset> = BTreeMap::new();
    let system = child(root, "System", true)?.expect("required=true never returns Ok(None)");
    let maps = child(root, "Maps", true)?.expect("required=true never returns Ok(None)");
    let textures = child(root, "Textures", true)?.expect("required=true never returns Ok(None)");
    let sounds = child(root, "Sounds", true)?.expect("required=true never returns Ok(None)");
    let music = child(root, "Music", true)?.expect("required=true never returns Ok(None)");
    let help = child(root, "Help", false)?;

    // Proactively Fail on a prohibited file sitting directly in System/
    // that the accept-filter below would otherwise just skip silently --
    // SourceAsset -> make_asset -> is_prohibited(dest) does the real
    // check; this loop only decides which files reach it. The
    // constructed Asset is intentionally discarded: this call exists
    // only for its potential error.
    for file in walk(&system, false)? {
        let name = utf8(&file.file_name().map(PathBuf::from).unwrap_or_default())?;
        if !is_debris(&file) && is_prohibited(&format!("System/{}", posix(&name))) {
            source_asset(source, root, &file, format!("System/{}", posix(&name)))?;
        }
    }

    collect_to(&mut assets, root, source, &system, "System", false, |p| {
        let ext = lower_extension(p);
        ext == ".u" || ext == ".int" || ext == ".usa"
    })?;
    collect_to(&mut assets, root, source, &maps, "Maps", true, |p| {
        lower_extension(p) == ".unr"
    })?;
    collect_to(
        &mut assets,
        root,
        source,
        &textures,
        "Textures",
        false,
        |p| lower_extension(p) == ".utx",
    )?;
    collect_to(&mut assets, root, source, &sounds, "Sounds", true, |p| {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .map(lower)
            .unwrap_or_default();
        name.ends_with(".uax") || name.ends_with("_uax")
    })?;
    collect_to(&mut assets, root, source, &music, "Music", true, |p| {
        lower_extension(p) == ".ogg"
    })?;
    if let Some(help) = help {
        collect_to(&mut assets, root, source, &help, "Help", false, |p| {
            !is_debris(p) && {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                !is_prohibited(name)
            }
        })?;
    }
    Ok(assets)
}

// All four cutscene collectors below were verified together against the
// real compiled C++ (`c++ main.cpp
// HarryPotter2/Unreal/Import/Src/HP2ImportCollect.cpp`, as for
// `collect_side`): byte-identical asset paths/sources/SHA-256 hashes on
// matching fixtures for all four, including the CutScenes-directory-
// absent empty-result case and the unexpected-payload error case.

/// Replaces `CollectRetailCutscenes`. Inserts into `assets` in place
/// (matching the C++ `&` out-parameter), source `"retail"`.
pub fn collect_retail_cutscenes(
    root: &Path,
    assets: &mut BTreeMap<String, Asset>,
) -> Result<(), String> {
    let system = child(root, "System", true)?.expect("required=true never returns Ok(None)");
    let cutscenes =
        child(&system, "cutscenes", true)?.expect("required=true never returns Ok(None)");
    for file in walk(&cutscenes, true)? {
        if lower_extension(&file) != ".int" {
            if !is_debris(&file) {
                return Err(fail(format!(
                    "unexpected retail cutscene payload: {}",
                    utf8(&file)?
                )));
            }
            continue;
        }
        let rel = lexically_relative(&cutscenes, &file).unwrap_or_default();
        let destination = format!("System/Cutscenes/{}", posix(&utf8(&rel)?));
        let asset = source_asset("retail", root, &file, destination)?;
        insert(assets, asset)?;
    }
    Ok(())
}

/// Replaces `CollectRetailRequiredNestedTextures`. Inserts into `assets`
/// in place, source `"retail"`.
pub fn collect_retail_required_nested_textures(
    root: &Path,
    assets: &mut BTreeMap<String, Asset>,
) -> Result<(), String> {
    let textures = child(root, "Textures", true)?.expect("required=true never returns Ok(None)");
    let old_textures =
        child(&textures, "Old textures", true)?.expect("required=true never returns Ok(None)");
    let menu = child_file(&old_textures, "HP_Menu.utx")?;
    let asset = source_asset("retail", root, &menu, "Textures/HP_Menu.utx".to_string())?;
    insert(assets, asset)
}

/// Replaces `CollectPrototypeRuntimeCutscenes`. Same filtering as
/// [`collect_retail_cutscenes`] but returns a fresh map, source
/// `"prototype"`, and a "prototype runtime cutscene" error message.
pub fn collect_prototype_runtime_cutscenes(root: &Path) -> Result<BTreeMap<String, Asset>, String> {
    let mut assets: BTreeMap<String, Asset> = BTreeMap::new();
    let system = child(root, "System", true)?.expect("required=true never returns Ok(None)");
    let cutscenes =
        child(&system, "cutscenes", true)?.expect("required=true never returns Ok(None)");
    for file in walk(&cutscenes, true)? {
        if lower_extension(&file) != ".int" {
            if !is_debris(&file) {
                return Err(fail(format!(
                    "unexpected prototype runtime cutscene payload: {}",
                    utf8(&file)?
                )));
            }
            continue;
        }
        let rel = lexically_relative(&cutscenes, &file).unwrap_or_default();
        let destination = format!("System/Cutscenes/{}", posix(&utf8(&rel)?));
        let asset = source_asset("prototype", root, &file, destination)?;
        insert(&mut assets, asset)?;
    }
    Ok(assets)
}

/// Replaces `CollectPrototypeCutsceneReferences`: an optional
/// `CutScenes/` directory of `.txt` cutscene *references* (not the
/// cutscene payload itself), each becoming a `System/Cutscenes/*.int`
/// destination -- `.int` replacing the source file's own extension, not
/// appended. Non-`.txt` files are silently skipped (no [`is_debris`]
/// check here, matching the C++ source exactly).
pub fn collect_prototype_cutscene_references(
    root: &Path,
) -> Result<BTreeMap<String, Asset>, String> {
    let mut assets: BTreeMap<String, Asset> = BTreeMap::new();
    let Some(cutscenes) = child(root, "CutScenes", false)? else {
        return Ok(assets);
    };
    for file in walk(&cutscenes, true)? {
        if lower_extension(&file) != ".txt" {
            continue;
        }
        let mut relative = lexically_relative(&cutscenes, &file).unwrap_or_default();
        relative.set_extension("int");
        let destination = format!("System/Cutscenes/{}", posix(&utf8(&relative)?));
        let asset = source_asset("prototype", root, &file, destination)?;
        insert(&mut assets, asset)?;
    }
    Ok(assets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self as stdfs, File};
    use std::io::Write;

    struct TempDir(PathBuf);
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = stdfs::remove_dir_all(&self.0);
        }
    }
    fn tempdir() -> TempDir {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "hp2-import-collect-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        stdfs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn write(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        stdfs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(path).unwrap().write_all(content).unwrap();
    }

    fn standard_tree(root: &Path) {
        for dir in ["System", "Maps", "Textures", "Sounds", "Music"] {
            stdfs::create_dir_all(root.join(dir)).unwrap();
        }
    }

    #[test]
    fn make_asset_hashes_and_records_relative_source_path() {
        let dir = tempdir();
        write(dir.path(), "System/Game.u", b"hello");
        let asset = make_asset(
            "prototype",
            dir.path(),
            &dir.path().join("System/Game.u"),
            "System/Game.u".to_string(),
        )
        .unwrap();
        assert_eq!(asset.source, "prototype");
        assert_eq!(asset.source_path, "System/Game.u");
        assert_eq!(asset.path, "System/Game.u");
        assert_eq!(asset.size, 5);
        // SHA-256("hello"), verified against Python hashlib.
        assert_eq!(
            asset.hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(asset.classification, "");
        assert!(!asset.generated);
        assert!(asset.sources.is_empty());
    }

    #[test]
    fn lexically_relative_computes_up_and_down_components_without_touching_the_filesystem() {
        assert_eq!(
            lexically_relative(Path::new("/a/b/c"), Path::new("/a/b/c/d/e")).unwrap(),
            Path::new("d/e")
        );
        assert_eq!(
            lexically_relative(Path::new("/a/b/c"), Path::new("/a/x/y")).unwrap(),
            Path::new("../../x/y")
        );
        assert_eq!(
            lexically_relative(Path::new("/a/b"), Path::new("/a/b")).unwrap(),
            Path::new("")
        );
        assert_eq!(
            lexically_relative(Path::new("/a/b"), Path::new("relative/path")),
            None
        );
    }

    #[test]
    fn make_asset_rejects_a_file_genuinely_outside_root() {
        // The bug this guards: a naive `strip_prefix(root).unwrap_or(file)`
        // silently falls back to the original (absolute, non-"../"-
        // prefixed) path on strip_prefix's Err instead of erroring, so
        // the "rel.is_empty() || starts_with(\"../\")" check never fires
        // and an out-of-root file sails through with an absolute path
        // left in source_path.
        let root = tempdir();
        let outside = tempdir();
        write(outside.path(), "secret.txt", b"x");
        let err = make_asset(
            "prototype",
            root.path(),
            &outside.path().join("secret.txt"),
            "System/secret.txt".to_string(),
        )
        .unwrap_err();
        assert!(
            err.starts_with("input asset escapes prototype root: "),
            "{err}"
        );
    }

    #[test]
    fn make_asset_rejects_a_prohibited_destination() {
        let dir = tempdir();
        write(dir.path(), "System/bad.exe", b"x");
        let err = make_asset(
            "prototype",
            dir.path(),
            &dir.path().join("System/bad.exe"),
            "System/bad.exe".to_string(),
        )
        .unwrap_err();
        assert_eq!(err, "prohibited asset selected for overlay: System/bad.exe");
    }

    #[test]
    #[cfg(unix)]
    fn make_asset_rejects_a_symlink() {
        let dir = tempdir();
        write(dir.path(), "real.txt", b"x");
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(dir.path().join("real.txt"), &link).unwrap();
        let err = make_asset("prototype", dir.path(), &link, "link.txt".to_string()).unwrap_err();
        assert!(
            err.starts_with("input asset is not a real regular file: "),
            "{err}"
        );
    }

    #[test]
    fn insert_rejects_a_case_insensitive_collision() {
        let mut assets = BTreeMap::new();
        let dir = tempdir();
        write(dir.path(), "a.txt", b"1");
        write(dir.path(), "b.txt", b"2");
        let first = make_asset(
            "prototype",
            dir.path(),
            &dir.path().join("a.txt"),
            "System/Thing.ini".to_string(),
        )
        .unwrap();
        let second = make_asset(
            "prototype",
            dir.path(),
            &dir.path().join("b.txt"),
            "SYSTEM/THING.INI".to_string(),
        )
        .unwrap();
        insert(&mut assets, first).unwrap();
        let err = insert(&mut assets, second).unwrap_err();
        assert_eq!(
            err,
            "case-insensitive output collision for SYSTEM/THING.INI: a.txt and b.txt"
        );
    }

    #[test]
    fn collect_side_gathers_recognized_extensions_under_canonical_destinations() {
        let dir = tempdir();
        standard_tree(dir.path());
        write(dir.path(), "System/Game.u", b"u");
        write(dir.path(), "System/Game.int", b"int");
        write(dir.path(), "System/ignored.txt", b"ignored");
        write(dir.path(), "Maps/PrivetDr.unr", b"map");
        write(dir.path(), "Textures/Wall.utx", b"tex");
        write(dir.path(), "Sounds/Door.uax", b"snd");
        write(dir.path(), "Sounds/Door_uax", b"snd2");
        write(dir.path(), "Music/theme.ogg", b"music");

        let assets = collect_side(dir.path(), "prototype").unwrap();
        let mut paths: Vec<&str> = assets.values().map(|a| a.path.as_str()).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                "Maps/PrivetDr.unr",
                "Music/theme.ogg",
                "Sounds/Door.uax",
                "Sounds/Door_uax",
                "System/Game.int",
                "System/Game.u",
                "Textures/Wall.utx",
            ]
        );
    }

    #[test]
    fn collect_side_fails_on_a_prohibited_file_directly_in_system() {
        let dir = tempdir();
        standard_tree(dir.path());
        write(dir.path(), "System/payload.exe", b"x");
        let err = collect_side(dir.path(), "prototype").unwrap_err();
        assert_eq!(
            err,
            "prohibited asset selected for overlay: System/payload.exe"
        );
    }

    #[test]
    fn collect_side_treats_help_as_optional() {
        let dir = tempdir();
        standard_tree(dir.path());
        let assets = collect_side(dir.path(), "prototype").unwrap();
        assert!(assets.is_empty());
    }

    #[test]
    fn collect_side_collects_help_when_present_excluding_debris_and_prohibited() {
        let dir = tempdir();
        standard_tree(dir.path());
        stdfs::create_dir_all(dir.path().join("Help")).unwrap();
        write(dir.path(), "Help/Readme.txt", b"help");
        write(dir.path(), "Help/Thumbs.db", b"debris");
        write(dir.path(), "Help/bad.exe", b"prohibited");

        let assets = collect_side(dir.path(), "prototype").unwrap();
        let mut paths: Vec<&str> = assets.values().map(|a| a.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["Help/Readme.txt"]);
    }

    #[test]
    fn collect_retail_cutscenes_gathers_int_files_and_skips_debris() {
        let dir = tempdir();
        stdfs::create_dir_all(dir.path().join("System/cutscenes/sub")).unwrap();
        write(dir.path(), "System/cutscenes/intro.int", b"c1");
        write(dir.path(), "System/cutscenes/sub/chapter2.int", b"c2");
        write(dir.path(), "System/cutscenes/Thumbs.db", b"debris");

        let mut assets = BTreeMap::new();
        collect_retail_cutscenes(dir.path(), &mut assets).unwrap();
        let mut paths: Vec<&str> = assets.values().map(|a| a.path.as_str()).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                "System/Cutscenes/intro.int",
                "System/Cutscenes/sub/chapter2.int"
            ]
        );
        assert_eq!(assets.values().next().unwrap().source, "retail");
    }

    #[test]
    fn collect_retail_cutscenes_fails_on_an_unexpected_non_debris_payload() {
        let dir = tempdir();
        stdfs::create_dir_all(dir.path().join("System/cutscenes")).unwrap();
        write(dir.path(), "System/cutscenes/stray.dat", b"x");
        let mut assets = BTreeMap::new();
        let err = collect_retail_cutscenes(dir.path(), &mut assets).unwrap_err();
        assert!(
            err.starts_with("unexpected retail cutscene payload: "),
            "{err}"
        );
    }

    #[test]
    fn collect_retail_required_nested_textures_finds_hp_menu_utx() {
        let dir = tempdir();
        stdfs::create_dir_all(dir.path().join("Textures/Old textures")).unwrap();
        write(dir.path(), "Textures/Old textures/HP_Menu.utx", b"menu");
        let mut assets = BTreeMap::new();
        collect_retail_required_nested_textures(dir.path(), &mut assets).unwrap();
        let asset = assets.values().next().unwrap();
        assert_eq!(asset.path, "Textures/HP_Menu.utx");
        assert_eq!(asset.source, "retail");
    }

    #[test]
    fn collect_prototype_runtime_cutscenes_uses_prototype_source_and_message() {
        let dir = tempdir();
        stdfs::create_dir_all(dir.path().join("System/cutscenes")).unwrap();
        write(dir.path(), "System/cutscenes/intro.int", b"c1");
        let assets = collect_prototype_runtime_cutscenes(dir.path()).unwrap();
        let asset = assets.values().next().unwrap();
        assert_eq!(asset.path, "System/Cutscenes/intro.int");
        assert_eq!(asset.source, "prototype");

        write(dir.path(), "System/cutscenes/stray.dat", b"x");
        let err = collect_prototype_runtime_cutscenes(dir.path()).unwrap_err();
        assert!(
            err.starts_with("unexpected prototype runtime cutscene payload: "),
            "{err}"
        );
    }

    #[test]
    fn collect_prototype_cutscene_references_is_optional_and_replaces_extension() {
        let dir = tempdir();
        // No CutScenes/ directory at all: empty result, not an error.
        assert!(
            collect_prototype_cutscene_references(dir.path())
                .unwrap()
                .is_empty()
        );

        stdfs::create_dir_all(dir.path().join("CutScenes/sub")).unwrap();
        write(dir.path(), "CutScenes/intro.txt", b"reference");
        write(dir.path(), "CutScenes/sub/chapter2.txt", b"reference2");
        // Non-.txt files are silently skipped, no Debris/error check.
        write(dir.path(), "CutScenes/notes.ini", b"ignored");

        let assets = collect_prototype_cutscene_references(dir.path()).unwrap();
        let mut paths: Vec<&str> = assets.values().map(|a| a.path.as_str()).collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                "System/Cutscenes/intro.int",
                "System/Cutscenes/sub/chapter2.int"
            ]
        );
        assert!(assets.values().all(|a| a.source == "prototype"));
    }
}
