//! Replaces `HP2ImportOutput.cpp` in full: `SafeDirectory`, `Overlap`,
//! `WalkOutput`, `ValidateOutput`, `Materialize`, `CommitStage`,
//! `Progress`, `Options`, `ProfileName`, and `RunImport` itself.
//!
//! [`safe_directory`] through [`commit_stage`] (plus [`Progress`])
//! operate on an already-built `&[Asset]` plan and real directories, so
//! (like [`crate::plan`]'s `validate_plan`) they need no retail/
//! prototype data tree -- every one of their own tests uses a
//! throwaway temp directory. [`run_import`] is the top-level
//! orchestration tying all of those together with [`crate::plan::plan`]
//! and (for the `--archive` input path) [`crate::archive::extract`];
//! porting it waited until both of those existed, since calling
//! functions that don't exist yet isn't an option. Its own tests follow
//! the same honest-scoping pattern [`crate::plan::plan`]'s doc explains
//! in full: the cheap argument-validation/overlap-check branches are
//! tested directly, and one more test proves the *entire* pipeline --
//! argument validation, output-path resolution (creating the output's
//! real parent directory), the retail/prototype root checks, the
//! overlap check, and finally `plan` itself -- runs correctly end to
//! end and the real `validate_plan` error comes back out through every
//! layer intact, not a full happy-path run (which would need a complete
//! synthetic retail+prototype tree satisfying every one of
//! `validate_plan`'s exact count contracts, disproportionate setup for
//! an environment that has no real data to test against anyway).
//!
//! `run_import`'s own end-to-end parity with the real C++ is -- like
//! `plan`'s -- not verified against real retail data, for the same
//! reason: no retail/prototype data tree exists anywhere in this repo.
//!
//! [`materialize`] checks `cancel_requested` before each asset (matching
//! C++'s per-asset `cancelRequested` check, "Import cancelled." on
//! trip) and increments `files_done` after each one (matching C++'s
//! `++progress->filesDone`); `run_import` sets `files_total`/
//! `files_done` once, right after `plan` returns, matching the real
//! C++'s own division of responsibility (`RunImport` sets them,
//! `Materialize` only increments the one).
//!
//! Three real divergences from the real C++ were found and fixed
//! during this port, each confirmed directly (a throwaway probe against
//! the real stdlib/compiler behavior for the first two; careful,
//! repeated empirical testing for the third -- see `crate::archive`'s
//! module doc for that one, caught in that module but structurally the
//! same kind of finding):
//! - `std::fs::copy` silently overwrites an existing destination; the
//!   real C++'s `fs::copy_file(..., fs::copy_options::none)` fails with
//!   "File exists" instead. [`copy_file_no_overwrite`] replicates the
//!   refusal.
//! - `std::fs::create_dir` errors with `AlreadyExists` even when the
//!   existing entry already is a directory; the real C++'s
//!   `fs::create_directory` is idempotent there (`created=false`, no
//!   error). [`create_directory_idempotent`] replicates the
//!   idempotency, used for `materialize`'s six category-root
//!   directories (the only single-level `create_dir` call in this
//!   module -- the per-asset `destination.parent()` creation already
//!   uses `create_dir_all`, confirmed independently idempotent in Rust
//!   too, matching C++'s equally-idempotent `create_directories`).
//!
//! [`lexically_normal`] (used by `run_import` to normalize the
//! `--output` argument before splitting it into parent+filename) has
//! lower verification confidence than the rest of this module: a
//! well-known standard algorithm, not a C++ transcription, and not
//! exercised by any test with a `..`-laden `--output` argument -- see
//! its own doc comment.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::classify::is_prohibited;
use crate::collect::Asset;
use crate::hash;
use crate::lower::lower;
use crate::manifest::checksums;
use crate::plan::LinkMode;
use crate::util::{fail, lexically_relative, strerror_of, utf8};
use hp2_ini::Profile;

fn display(p: &Path) -> String {
    utf8(p).unwrap_or_else(|_| p.display().to_string())
}

/// Replaces `RandomHex()`: 32 lowercase hex characters (128 bits),
/// non-deterministic (as the C++ `std::random_device`-backed original
/// is too) -- used only for temp-directory-name uniqueness, never
/// written to any file this crate's parity tests compare, so no
/// cross-impl byte matching is required, just "doesn't collide in
/// practice." Same `RandomState`-based technique [`crate::plan`]'s
/// `random_probe_name` already uses for the same reason (this crate has
/// no `rand` dependency).
fn random_hex() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let a = RandomState::new().build_hasher().finish();
    let b = RandomState::new().build_hasher().finish();
    format!("{a:016x}{b:016x}")
}

/// Replaces `SafeDirectory`: resolves `path` to a canonical, real
/// (non-symlink) directory, or fails with `label` naming which
/// command-line root the problem is in.
pub fn safe_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        return Err(fail(format!(
            "{label} must not be a symbolic link: {}",
            display(path)
        )));
    }
    let resolved = fs::canonicalize(path).map_err(|e| {
        fail(format!(
            "cannot resolve {label} {}: {}",
            display(path),
            strerror_of(&e)
        ))
    })?;
    if !resolved.is_dir() {
        return Err(fail(format!(
            "{label} is not a directory: {}",
            display(&resolved)
        )));
    }
    Ok(resolved)
}

/// Replaces `Overlap`: true when `a` is lexically inside `b`, `b` is
/// lexically inside `a`, or they are the same canonical path.
///
/// [`lexically_relative`] returns an *empty* `PathBuf` for identical
/// paths (zero `..` components, zero trailing components -- the only
/// way to get both is `base == path`), where C++'s own
/// `fs::path::lexically_relative` returns `"."` for the same case. Both
/// have a first component that is not `".."`, so both read as overlap
/// under the C++ logic this replaces (`!relative.empty() &&
/// *relative.begin() != ".."` is true for `"."`, and is true here for
/// empty too, by the explicit `is_empty()` branch below) -- same
/// outcome, different empty-vs-dot representation of "no path left to
/// strip."
pub fn overlap(a: &Path, b: &Path) -> bool {
    let contains = |base: &Path, path: &Path| {
        lexically_relative(base, path).is_some_and(|rel| {
            rel.as_os_str().is_empty() || rel.components().next() != Some(Component::ParentDir)
        })
    };
    contains(b, a) || contains(a, b)
}

/// Replaces `WalkOutput`. Unlike [`crate::files::deterministic_files`]/
/// [`crate::files::walk`], the real C++ `WalkOutput` uses a plain
/// `fs::recursive_directory_iterator` with no `std::sort` anywhere --
/// its traversal order is OS/filesystem-dependent, not a documented,
/// reproducible sort the way `DeterministicFiles`/`Walk` are. When more
/// than one output entry has a problem, *which* one `WalkOutput` reports
/// first is therefore not a fixed, matchable property of the C++ itself
/// -- there is no canonical order here to replicate. This walks via a
/// plain `fs::read_dir`-ordered stack (not re-sorted), which is exactly
/// as order-stable (or not) as the C++ it replaces for that reason, not
/// less faithful than a sorted alternative would be.
fn walk_output(output: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let bad_output = || {
        fail(format!(
            "output is not a real directory: {}",
            display(output)
        ))
    };
    let meta = fs::symlink_metadata(output).map_err(|_| bad_output())?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(bad_output());
    }
    let mut found: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut pending = vec![output.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| {
            fail(format!(
                "cannot inspect output {}: {}",
                display(&dir),
                strerror_of(&e)
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| {
                fail(format!(
                    "cannot inspect output {}: {}",
                    display(&dir),
                    strerror_of(&e)
                ))
            })?;
            let path = entry.path();
            let relative = lexically_relative(output, &path)
                .ok_or_else(|| fail(format!("cannot inspect output {}", display(&path))))?;
            let relative_str = utf8(&relative)?;
            let entry_meta = entry.metadata().map_err(|e| {
                fail(format!(
                    "cannot stat output {}: {}",
                    display(&path),
                    strerror_of(&e)
                ))
            })?;
            if entry_meta.file_type().is_symlink() {
                return Err(fail(format!(
                    "symbolic link in output (dangling links are forbidden): {}",
                    display(&path)
                )));
            }
            if entry_meta.is_dir() {
                pending.push(path);
                continue;
            }
            if !entry_meta.is_file() {
                return Err(fail(format!(
                    "non-regular output entry: {}",
                    display(&path)
                )));
            }
            let key = lower(&relative_str);
            if let Some(prior) = found.get(&key) {
                return Err(fail(format!(
                    "case-insensitive output collision: {} and {}",
                    display(prior),
                    display(&path)
                )));
            }
            found.insert(key, path.clone());
            if is_prohibited(&relative_str) {
                return Err(fail(format!("prohibited output payload: {relative_str}")));
            }
            let mut file = fs::File::open(&path).map_err(|_| {
                fail(format!(
                    "cannot inspect output {}: cannot open file",
                    display(&path)
                ))
            })?;
            let mut signature = [0u8; 2];
            let mut filled = 0usize;
            loop {
                match file.read(&mut signature[filled..]) {
                    Ok(0) => break,
                    Ok(n) => {
                        filled += n;
                        if filled == 2 {
                            break;
                        }
                    }
                    Err(e) => {
                        return Err(fail(format!(
                            "cannot inspect output {}: read error: {}",
                            display(&path),
                            strerror_of(&e)
                        )));
                    }
                }
            }
            if filled == 2 && signature == *b"MZ" {
                return Err(fail(format!(
                    "Windows PE payload disguised in output: {relative_str}"
                )));
            }
        }
    }
    Ok(found)
}

/// Replaces `ValidateOutput`: confirms `output` contains exactly the
/// expected set of asset files plus `overlay-manifest.json`/
/// `overlay-checksums.txt`, each under its exact canonical-case path,
/// with up-to-date identity files and every asset's on-disk size/hash
/// matching the plan.
pub fn validate_output(output: &Path, assets: &[Asset], manifest: &str) -> Result<(), String> {
    let found = walk_output(output)?;
    let mut expected: BTreeMap<String, Option<&Asset>> = BTreeMap::new();
    for asset in assets {
        expected.insert(lower(&asset.path), Some(asset));
    }
    expected.insert("overlay-manifest.json".to_string(), None);
    expected.insert("overlay-checksums.txt".to_string(), None);
    let missing: Vec<&String> = expected
        .keys()
        .filter(|k| !found.contains_key(*k))
        .collect();
    let extra: Vec<&String> = found
        .keys()
        .filter(|k| !expected.contains_key(*k))
        .collect();
    if !missing.is_empty() || !extra.is_empty() {
        let list = |values: &[&String]| {
            let items: Vec<String> = values.iter().map(|v| format!("'{v}'")).collect();
            format!("[{}]", items.join(", "))
        };
        return Err(fail(format!(
            "output path set mismatch: missing={}, extra={}",
            list(&missing),
            list(&extra)
        )));
    }
    let canonical_name = |key: &str,
                          expected_rel: &str,
                          description: &str|
     -> Result<PathBuf, String> {
        let path = found
            .get(key)
            .expect("key presence already checked by the missing/extra pass above");
        let relative = lexically_relative(output, path)
            .ok_or_else(|| fail(format!("cannot inspect output {}", display(path))))?;
        let relative_str = utf8(&relative)?;
        if relative_str != expected_rel {
            return Err(fail(format!(
                "{description} has incorrect canonical case: {relative_str}, expected {expected_rel}"
            )));
        }
        Ok(path.clone())
    };
    let manifest_path =
        canonical_name("overlay-manifest.json", "overlay-manifest.json", "manifest")?;
    let checksums_path = canonical_name(
        "overlay-checksums.txt",
        "overlay-checksums.txt",
        "checksums index",
    )?;
    for asset in assets {
        canonical_name(&lower(&asset.path), &asset.path, "output")?;
    }
    let read_file = |path: &Path, label: &str| -> Result<Vec<u8>, String> {
        let mut file = fs::File::open(path).map_err(|_| {
            fail(format!(
                "cannot read {label} {}: cannot open file",
                display(path)
            ))
        })?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|_| fail(format!("cannot read {label} {}: read error", display(path))))?;
        Ok(bytes)
    };
    if read_file(&manifest_path, "manifest")? != manifest.as_bytes() {
        return Err(fail(format!(
            "manifest is out of date or non-deterministic: {}",
            display(&manifest_path)
        )));
    }
    if read_file(&checksums_path, "checksums index")? != checksums(assets).as_bytes() {
        return Err(fail(format!(
            "checksums index is out of date or non-deterministic: {}",
            display(&checksums_path)
        )));
    }
    for asset in assets {
        let path = found
            .get(&lower(&asset.path))
            .expect("key presence already checked by the missing/extra pass above");
        let (size, found_hash) = hash::hash(path)?;
        if size != asset.size || found_hash != asset.hash {
            return Err(fail(format!(
                "output hash mismatch for {}: expected {}/{}, found {}/{}",
                asset.path, asset.size, asset.hash, size, found_hash
            )));
        }
    }
    Ok(())
}

/// Replaces `Progress`: shared, atomically-updated file-count and
/// cancellation state a caller can poll/set from another thread while
/// [`materialize`]/[`run_import`] run. `run_import` sets `files_total`/
/// `files_done` once, right after `plan` returns, matching the real
/// C++'s own division of responsibility (`RunImport` sets them,
/// `Materialize` only increments `filesDone`).
///
/// `#[repr(C)]`: this struct is shared directly across the FFI
/// boundary with C++'s own `HP2Import::Progress`
/// (`std::atomic<std::int64_t> filesDone, filesTotal; std::atomic<bool>
/// cancelRequested;`) via `hp2-ffi`'s `hp2_rs_import_run` (a `void*`
/// the real C++ caller casts from its own `HP2Import::Progress*`, so
/// no cross-crate struct needs to appear in the generated header).
/// Confirmed layout-compatible by direct measurement, not just by the
/// general claim that `AtomicI64`/`AtomicBool` have the same size,
/// alignment, and bit validity as their underlying integer types and a
/// lock-free `std::atomic<T>` is just `T` under the hood: a throwaway
/// probe on each side found `sizeof`/`size_of` = 24,
/// `alignof`/`align_of` = 8, and identical field offsets (0, 8, 16) on
/// both the real `HP2Import::Progress` and this struct.
#[repr(C)]
#[derive(Default)]
pub struct Progress {
    pub files_done: std::sync::atomic::AtomicI64,
    pub files_total: std::sync::atomic::AtomicI64,
    pub cancel_requested: std::sync::atomic::AtomicBool,
}

/// Matches `fs::copy_file(from, to, fs::copy_options::none)`: unlike
/// `std::fs::copy`, which silently overwrites an existing `to` (Rust
/// stdlib docs: "If `to` already exists, it will be overwritten" --
/// confirmed directly against a throwaway probe: `std::fs::copy` onto
/// an existing destination succeeds, while the real C++
/// `copy_file(..., copy_options::none)` fails with "File exists"),
/// `copy_options::none` is the *non*-overwriting default the real
/// `Materialize` uses at every copy call site. Every `materialize` call
/// site writes into a fresh `stage` directory where no destination
/// should already exist in well-formed use, but matching the refusal
/// (not silently overwriting) is the correct behavior for "something
/// already placed a file at this destination," the case the C++ guards
/// against.
fn copy_file_no_overwrite(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut source = fs::File::open(from)?;
    let mut dest = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    std::io::copy(&mut source, &mut dest)?;
    Ok(())
}

/// Matches `fs::create_directory`'s idempotency: succeeds silently if
/// `dir` already exists as a directory. Confirmed via a throwaway
/// probe that this differs from Rust's `fs::create_dir`, which errors
/// with `AlreadyExists` even when the existing entry already is a
/// directory (C++'s `create_directory` returns `false`/no error in
/// that case, only erroring for a genuine failure such as the target
/// existing as a non-directory or the parent being missing). Only used
/// for the six standard per-asset-category roots `materialize` creates
/// directly in `stage`; the per-asset `destination.parent()` creation
/// below already uses `fs::create_dir_all`, which -- also confirmed via
/// a throwaway probe -- is already idempotent in Rust, matching C++'s
/// equally-idempotent `create_directories` with no adjustment needed.
fn create_directory_idempotent(dir: &Path) -> std::io::Result<()> {
    match fs::create_dir(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        Err(e) => Err(e),
    }
}

/// Replaces `Materialize`. `stage` must already exist (an empty
/// directory): the real C++ creates it in `RunImport`, immediately
/// before calling `Materialize`, so this keeps the same precondition
/// rather than silently creating it here -- a caller skipping that step
/// gets the same kind of "cannot create staging directory: already
/// exists"-shaped `io::Error` the C++ would also surface at that point
/// (from the first `fs::create_directory(stage / root)` call), not a
/// different failure mode.
pub fn materialize(
    stage: &Path,
    assets: &[Asset],
    manifest: &str,
    link_mode: LinkMode,
    progress: Option<&Progress>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    for root in ["System", "Maps", "Textures", "Sounds", "Music", "Help"] {
        let dir = stage.join(root);
        create_directory_idempotent(&dir).map_err(|e| {
            fail(format!(
                "cannot create staging directory {}: {}",
                display(&dir),
                strerror_of(&e)
            ))
        })?;
    }
    for asset in assets {
        if let Some(p) = progress
            && p.cancel_requested.load(Ordering::Relaxed)
        {
            return Err(fail("Import cancelled."));
        }
        let destination = stage.join(&asset.path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                fail(format!(
                    "cannot materialize {}: {}",
                    asset.path,
                    strerror_of(&e)
                ))
            })?;
        }
        if asset.generated {
            fs::write(&destination, asset.content.as_bytes())
                .map_err(|_| fail(format!("cannot materialize {}: write failed", asset.path)))?;
        } else if link_mode != LinkMode::Copy {
            match fs::hard_link(&asset.disk, &destination) {
                Ok(()) => {}
                Err(e) if link_mode == LinkMode::Hardlink => {
                    return Err(fail(format!(
                        "cannot materialize {}: {}",
                        asset.path,
                        strerror_of(&e)
                    )));
                }
                Err(_) => {
                    copy_file_no_overwrite(&asset.disk, &destination).map_err(|e| {
                        fail(format!(
                            "cannot materialize {}: {}",
                            asset.path,
                            strerror_of(&e)
                        ))
                    })?;
                }
            }
        } else {
            copy_file_no_overwrite(&asset.disk, &destination).map_err(|e| {
                fail(format!(
                    "cannot materialize {}: {}",
                    asset.path,
                    strerror_of(&e)
                ))
            })?;
        }
        if let Some(p) = progress {
            p.files_done.fetch_add(1, Ordering::Relaxed);
        }
    }
    fs::write(stage.join("overlay-manifest.json"), manifest.as_bytes())
        .map_err(|_| fail("cannot write overlay identity files"))?;
    fs::write(
        stage.join("overlay-checksums.txt"),
        checksums(assets).as_bytes(),
    )
    .map_err(|_| fail("cannot write overlay identity files"))?;
    Ok(())
}

/// Replaces `CommitStage`: atomically installs `stage` as `output`,
/// backing up and restoring any prior `output` on failure.
pub fn commit_stage(stage: &Path, output: &Path) -> Result<(), String> {
    let status = fs::symlink_metadata(output);
    let exists = status.is_ok();
    if exists {
        let meta = status.expect("checked by `exists` above");
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(fail(format!(
                "refusing to replace non-directory output: {}",
                display(output)
            )));
        }
        let parent = output.parent().unwrap_or_else(|| Path::new(""));
        let name = output
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let backup = parent.join(format!(".{name}.old-{}", random_hex()));
        fs::rename(output, &backup).map_err(|e| {
            fail(format!(
                "cannot atomically replace {}: {}",
                display(output),
                strerror_of(&e)
            ))
        })?;
        if let Err(e) = fs::rename(stage, output) {
            let reason = strerror_of(&e);
            let _ = fs::rename(&backup, output);
            return Err(fail(format!(
                "cannot atomically replace {}: {reason}",
                display(output)
            )));
        }
        fs::remove_dir_all(&backup).map_err(|e| {
            fail(format!(
                "cannot remove replaced output backup {}: {}",
                display(&backup),
                strerror_of(&e)
            ))
        })?;
    } else {
        fs::rename(stage, output).map_err(|e| {
            fail(format!(
                "cannot atomically install {}: {}",
                display(output),
                strerror_of(&e)
            ))
        })?;
    }
    Ok(())
}

/// Mirrors `HarryPotter2/Unreal/Import/Inc/HP2Import.h`'s `struct
/// Options`. The whole bundle is only needed here, by `run_import`
/// itself -- every other function in this crate so far takes just the
/// specific fields it reads directly (see this module's and `plan`'s
/// own doc comments for why), the same way the real C++'s own
/// `Options` only exists to be passed around as a bundle by
/// `RunImport`'s own callers.
pub struct Options {
    pub retail_root: String,
    pub archive: String,
    pub prototype_root: String,
    pub output: String,
    pub profile: Profile,
    pub link_mode: LinkMode,
    pub check_only: bool,
}

/// Replaces `ProfileName`.
fn profile_name(profile: Profile) -> &'static str {
    match profile {
        Profile::Safe => "safe",
        Profile::Full => "full",
        Profile::RetailOnly => "retail-only",
    }
}

/// Replaces `fs::path::lexically_normal()`: resolves `.`/`..`
/// components and redundant separators purely lexically, without
/// touching the filesystem (so it works on a path that may not exist
/// yet, unlike `canonicalize`). A `..` cancels the immediately
/// preceding `Normal` component if there is one; otherwise (at the
/// start of a relative path, or following another un-cancelled `..`)
/// it is kept literally. Lower verification confidence than the rest
/// of this port: this is a well-known, standard algorithm (the same
/// one `realpath -s`/most "lexical path clean" implementations use),
/// not a line-by-line C++ transcription, and `run_import`'s own tests
/// don't exercise a `..`-laden `--output` argument (real callers
/// overwhelmingly pass a plain relative or already-clean absolute
/// path) -- flagged, not silently assumed equivalent to the C++ in
/// every edge case.
fn lexically_normal(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match result.components().next_back() {
                Some(Component::Normal(_)) => {
                    result.pop();
                }
                _ => result.push(".."),
            },
            other => result.push(other.as_os_str()),
        }
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}

/// Dispatches to the real [`crate::archive::extract`] when the
/// `archive` Cargo feature is enabled, or fails with the same
/// user-facing message the real C++ gives its `__EMSCRIPTEN__` build
/// (generalized: "not compiled into this build" covers both a future
/// wasm target and simply building without the feature, which is the
/// only way this branch is reachable today since no wasm target exists
/// yet for this crate).
fn extract_archive(archive_path: &Path, destination: &Path) -> Result<PathBuf, String> {
    #[cfg(feature = "archive")]
    {
        crate::archive::extract(archive_path, destination)
    }
    #[cfg(not(feature = "archive"))]
    {
        let _ = (archive_path, destination);
        Err(fail(
            "Archives are not supported in this build; extract the archive and choose the folder.",
        ))
    }
}

/// Replaces `RunImport`. Returns `Ok(log)` (always `"Import completed
/// successfully.\n"`, matching the C++'s one success message) or
/// `Err(message)`, replacing the C++'s `bool` return plus `Log`/`Error`
/// out-parameters with this crate's established `Result<String,
/// String>` convention.
///
/// `archive_temporary`'s cleanup is restructured from the C++'s six
/// separate `if (!archiveTemporary.empty()) fs::remove_all(...)` call
/// sites (one before each of the three success returns, one in each of
/// the two catch blocks) into one, run exactly once after the inner
/// logic finishes regardless of outcome -- equivalent because every one
/// of the six C++ call sites does the exact same unconditional cleanup,
/// not because any of them behaves differently from the others.
pub fn run_import(options: &Options, progress: Option<&Progress>) -> Result<String, String> {
    let mut archive_temporary: Option<PathBuf> = None;
    let result = run_import_inner(options, progress, &mut archive_temporary);
    if let Some(temp) = &archive_temporary {
        let _ = fs::remove_dir_all(temp);
    }
    result
}

fn run_import_inner(
    options: &Options,
    progress: Option<&Progress>,
    archive_temporary: &mut Option<PathBuf>,
) -> Result<String, String> {
    use std::sync::atomic::Ordering;

    if options.retail_root.is_empty() == options.archive.is_empty() {
        return Err(fail(
            "exactly one of --retail-root and --archive is required",
        ));
    }
    if options.output.is_empty() {
        return Err(fail("--output is required"));
    }
    if options.profile != Profile::RetailOnly && options.prototype_root.is_empty() {
        return Err(fail(
            "--prototype-root is required for safe and full profiles",
        ));
    }

    let mut output = PathBuf::from(&options.output);
    if !output.is_absolute() {
        let cwd = std::env::current_dir().map_err(|e| {
            fail(format!(
                "cannot resolve current directory: {}",
                strerror_of(&e)
            ))
        })?;
        output = cwd.join(output);
    }
    let output = lexically_normal(&output);
    let parent = output
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(""));
    if options.check_only {
        let ok = fs::symlink_metadata(&parent)
            .map(|m| !m.file_type().is_symlink() && m.is_dir())
            .unwrap_or(false);
        if !ok {
            return Err(fail(format!(
                "output parent does not exist for --check: {}",
                display(&parent)
            )));
        }
    } else {
        fs::create_dir_all(&parent).map_err(|e| {
            fail(format!(
                "cannot prepare output parent {}: {}",
                display(&parent),
                strerror_of(&e)
            ))
        })?;
    }
    let resolved_parent = fs::canonicalize(&parent).map_err(|e| {
        fail(format!(
            "cannot prepare output parent {}: {}",
            display(&parent),
            strerror_of(&e)
        ))
    })?;
    let output = resolved_parent.join(output.file_name().unwrap_or_default());

    let mut retail: Option<PathBuf> = None;
    if !options.archive.is_empty() {
        let archive_path = PathBuf::from(&options.archive);
        let is_regular_file = fs::symlink_metadata(&archive_path)
            .map(|m| !m.file_type().is_symlink() && m.is_file())
            .unwrap_or(false);
        if !is_regular_file {
            return Err(fail(format!(
                "retail archive is not a regular file: {}",
                display(&archive_path)
            )));
        }
        let temp = resolved_parent.join(format!(".hp2-import-archive-{}", random_hex()));
        fs::create_dir(&temp).map_err(|e| {
            fail(format!(
                "cannot create archive staging directory {}: {}",
                display(&temp),
                strerror_of(&e)
            ))
        })?;
        *archive_temporary = Some(temp.clone());
        retail = Some(extract_archive(&archive_path, &temp)?);
    }
    let retail = match retail {
        Some(r) => r,
        None => safe_directory(Path::new(&options.retail_root), "retail root")?,
    };
    let mut prototype: Option<PathBuf> = None;
    if !options.prototype_root.is_empty() {
        prototype = Some(safe_directory(
            Path::new(&options.prototype_root),
            "prototype root",
        )?);
    }
    if prototype.as_deref().is_some_and(|p| overlap(&output, p)) || overlap(&output, &retail) {
        return Err(fail("output and source trees must not overlap"));
    }

    let assets = crate::plan::plan(
        options.profile,
        options.link_mode,
        prototype.as_deref(),
        &retail,
        &resolved_parent,
    )?;
    let manifest = crate::manifest::manifest(&assets, profile_name(options.profile));
    if let Some(p) = progress {
        p.files_total.store(assets.len() as i64, Ordering::Relaxed);
        p.files_done.store(0, Ordering::Relaxed);
    }

    if options.check_only {
        validate_output(&output, &assets, &manifest)?;
        return Ok("Import completed successfully.\n".to_string());
    }
    if (output.exists() || output.is_symlink())
        && validate_output(&output, &assets, &manifest).is_ok()
    {
        return Ok("Import completed successfully.\n".to_string());
    }

    let stage_name = format!(
        ".{}.tmp-{}",
        display(Path::new(output.file_name().unwrap_or_default())),
        random_hex()
    );
    let stage = resolved_parent.join(stage_name);
    fs::create_dir(&stage).map_err(|e| {
        fail(format!(
            "cannot create staging directory {}: {}",
            display(&stage),
            strerror_of(&e)
        ))
    })?;
    let commit_result: Result<(), String> = (|| {
        materialize(&stage, &assets, &manifest, options.link_mode, progress)?;
        validate_output(&stage, &assets, &manifest)?;
        if let Some(p) = progress
            && p.cancel_requested.load(Ordering::Relaxed)
        {
            return Err(fail("Import cancelled."));
        }
        commit_stage(&stage, &output)
    })();
    if let Err(e) = commit_result {
        let _ = fs::remove_dir_all(&stage);
        return Err(e);
    }
    Ok("Import completed successfully.\n".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_support::{tempdir, write};

    fn minimal_asset(path: &str, disk: &Path) -> Asset {
        Asset {
            path: path.to_string(),
            disk: disk.to_path_buf(),
            ..Asset::default()
        }
    }

    // -- overlap -----------------------------------------------------

    /// Every case here confirmed against the real compiled `Overlap`
    /// (verbatim-copied into a throwaway probe TU, since `Overlap` has
    /// internal linkage in the real C++ -- no header declares it).
    #[test]
    fn overlap_cases_match_cpp() {
        assert!(
            overlap(Path::new("/tmp/x/a"), Path::new("/tmp/x/a")),
            "identical paths"
        );
        assert!(
            overlap(Path::new("/tmp/x/a/b"), Path::new("/tmp/x/a")),
            "nested"
        );
        assert!(
            overlap(Path::new("/tmp/x/a"), Path::new("/tmp/x/a/b")),
            "reverse nested"
        );
        assert!(
            !overlap(Path::new("/tmp/x/a"), Path::new("/tmp/x/a2")),
            "siblings"
        );
        assert!(
            !overlap(Path::new("/tmp/x/a"), Path::new("/tmp/y/a")),
            "unrelated roots"
        );
    }

    // -- safe_directory ------------------------------------------------

    #[test]
    fn safe_directory_resolves_a_real_directory() {
        let dir = tempdir();
        let resolved = safe_directory(dir.path(), "test root").unwrap();
        assert_eq!(resolved, fs::canonicalize(dir.path()).unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn safe_directory_rejects_a_symlink() {
        let dir = tempdir();
        let real = dir.path().join("realdir");
        fs::create_dir(&real).unwrap();
        let link = dir.path().join("symlinked");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = safe_directory(&link, "test root").unwrap_err();
        assert!(
            err.starts_with("test root must not be a symbolic link: "),
            "{err}"
        );
    }

    #[test]
    fn safe_directory_rejects_a_non_directory() {
        let dir = tempdir();
        write(dir.path(), "afile", b"x");
        let err = safe_directory(&dir.path().join("afile"), "test root").unwrap_err();
        assert!(err.starts_with("test root is not a directory: "), "{err}");
    }

    #[test]
    fn safe_directory_reports_a_missing_path() {
        let dir = tempdir();
        let err = safe_directory(&dir.path().join("nope"), "test root").unwrap_err();
        assert!(err.starts_with("cannot resolve test root "), "{err}");
        assert!(!err.contains("os error"), "{err}");
    }

    // -- materialize -----------------------------------------------------

    #[test]
    fn materialize_copy_mode_writes_disk_and_generated_assets_plus_identity_files() {
        let src = tempdir();
        write(src.path(), "core.u", b"hello world");
        let stage = tempdir();
        let assets = vec![
            minimal_asset("System/Core.u", &src.path().join("core.u")),
            Asset {
                path: "System/Default.ini".to_string(),
                generated: true,
                content: "generated text".to_string(),
                ..Asset::default()
            },
        ];
        materialize(stage.path(), &assets, "MANIFEST\n", LinkMode::Copy, None).unwrap();
        assert_eq!(
            fs::read(stage.path().join("System/Core.u")).unwrap(),
            b"hello world"
        );
        assert_eq!(
            fs::read_to_string(stage.path().join("System/Default.ini")).unwrap(),
            "generated text"
        );
        assert_eq!(
            fs::read_to_string(stage.path().join("overlay-manifest.json")).unwrap(),
            "MANIFEST\n"
        );
        assert_eq!(
            fs::read_to_string(stage.path().join("overlay-checksums.txt")).unwrap(),
            checksums(&assets)
        );
    }

    /// Confirmed against real compiled `Materialize`: hardlink mode
    /// places a genuine hard link (same inode), not a copy.
    #[test]
    #[cfg(unix)]
    fn materialize_hardlink_mode_creates_a_real_hard_link() {
        use std::os::unix::fs::MetadataExt;
        let src = tempdir();
        write(src.path(), "core.u", b"hello world");
        let stage = tempdir();
        let assets = vec![minimal_asset("System/Core.u", &src.path().join("core.u"))];
        materialize(
            stage.path(),
            &assets,
            "MANIFEST\n",
            LinkMode::Hardlink,
            None,
        )
        .unwrap();
        let src_ino = fs::metadata(src.path().join("core.u")).unwrap().ino();
        let dst_ino = fs::metadata(stage.path().join("System/Core.u"))
            .unwrap()
            .ino();
        assert_eq!(src_ino, dst_ino);
    }

    /// Matches C++: `stage` must already exist (created by the caller,
    /// `RunImport` in the real C++) -- `materialize` itself only creates
    /// the six category subdirectories under it, not `stage` itself.
    #[test]
    fn materialize_fails_if_stage_itself_does_not_exist() {
        let parent = tempdir();
        let stage = parent.path().join("does-not-exist");
        let err = materialize(&stage, &[], "M\n", LinkMode::Auto, None).unwrap_err();
        assert!(err.starts_with("cannot create staging directory "), "{err}");
    }

    /// Confirmed against real compiled `Materialize` with
    /// `progress->cancelRequested = true`: fails with exactly
    /// "Import cancelled." before touching any asset, materializing
    /// nothing.
    #[test]
    fn materialize_stops_immediately_when_cancel_requested() {
        let src = tempdir();
        write(src.path(), "core.u", b"x");
        let stage = tempdir();
        let assets = vec![minimal_asset("System/Core.u", &src.path().join("core.u"))];
        let progress = Progress::default();
        progress
            .cancel_requested
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let err = materialize(
            stage.path(),
            &assets,
            "M\n",
            LinkMode::Auto,
            Some(&progress),
        )
        .unwrap_err();
        assert_eq!(err, "Import cancelled.");
        assert!(!stage.path().join("System/Core.u").exists());
    }

    /// Real, confirmed divergence caught and fixed during this port:
    /// `std::fs::copy` silently overwrites an existing destination
    /// (confirmed via a throwaway probe), unlike the real C++'s
    /// `fs::copy_file(..., fs::copy_options::none)`, which fails with
    /// "File exists" (also confirmed, both via a throwaway probe and
    /// against the real compiled `Materialize` on this exact scenario).
    /// [`copy_file_no_overwrite`] closes that gap; this pins the
    /// resulting behavior and that the pre-existing destination's
    /// content survives the failed attempt untouched.
    #[test]
    fn materialize_refuses_to_overwrite_an_existing_destination_file() {
        let src = tempdir();
        write(src.path(), "core.u", b"new content");
        let stage = tempdir();
        write(stage.path(), "System/Core.u", b"PRE-EXISTING");
        let assets = vec![minimal_asset("System/Core.u", &src.path().join("core.u"))];
        let err = materialize(stage.path(), &assets, "M\n", LinkMode::Copy, None).unwrap_err();
        assert_eq!(err, "cannot materialize System/Core.u: File exists");
        assert_eq!(
            fs::read(stage.path().join("System/Core.u")).unwrap(),
            b"PRE-EXISTING"
        );
    }

    /// Real, confirmed divergence caught and fixed during this port:
    /// Rust's `fs::create_dir` errors with `AlreadyExists` even when the
    /// existing entry already is a directory, unlike the real C++'s
    /// `fs::create_directory`, which is idempotent there (confirmed via
    /// a throwaway probe: `created=false`, no error). Without
    /// [`create_directory_idempotent`], a `stage` directory that already
    /// has one of the six category subdirectories present (e.g. created
    /// by an earlier partial run) would fail immediately instead of
    /// proceeding, matching the real C++ in this case too.
    #[test]
    fn materialize_tolerates_a_pre_existing_category_subdirectory() {
        let stage = tempdir();
        fs::create_dir(stage.path().join("System")).unwrap();
        materialize(stage.path(), &[], "M\n", LinkMode::Auto, None).unwrap();
    }

    // -- validate_output -----------------------------------------------------

    /// Shared fixture: materializes two assets for real, then returns
    /// `(stage, assets)` with each asset's `size`/`hash` set to the
    /// actual on-disk content's real values -- so `validate_output` on
    /// this fixture, unmodified, is a genuine happy path.
    fn materialized_fixture() -> (crate::util::test_support::TempDir, Vec<Asset>) {
        let src = tempdir();
        write(src.path(), "core.u", b"hello world!");
        let stage = tempdir();
        let (core_size, core_hash) = hash::hash(&src.path().join("core.u")).unwrap();
        let generated_content = "generated text";
        let mut assets = vec![
            minimal_asset("System/Core.u", &src.path().join("core.u")),
            Asset {
                path: "System/Default.ini".to_string(),
                generated: true,
                content: generated_content.to_string(),
                size: generated_content.len() as u64,
                hash: hash::hash_bytes(generated_content.as_bytes()),
                ..Asset::default()
            },
        ];
        assets[0].size = core_size;
        assets[0].hash = core_hash;
        materialize(stage.path(), &assets, "MANIFEST\n", LinkMode::Copy, None).unwrap();
        (stage, assets)
    }

    #[test]
    fn validate_output_accepts_a_genuinely_matching_output() {
        let (stage, assets) = materialized_fixture();
        validate_output(stage.path(), &assets, "MANIFEST\n").unwrap();
    }

    #[test]
    fn validate_output_reports_a_missing_asset() {
        let (stage, mut assets) = materialized_fixture();
        assets.push(minimal_asset("System/Extra.ini", Path::new("/nonexistent")));
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert_eq!(
            err,
            "output path set mismatch: missing=['system/extra.ini'], extra=[]"
        );
    }

    #[test]
    fn validate_output_reports_an_extra_file() {
        let (stage, mut assets) = materialized_fixture();
        assets.remove(1); // Default.ini stays on disk but drops out of the expected set.
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert_eq!(
            err,
            "output path set mismatch: missing=[], extra=['system/default.ini']"
        );
    }

    /// The specific branch flagged as unexercised during review: a
    /// claimed `hash`/`size` that is internally self-consistent with
    /// `overlay-checksums.txt` (so the checksums-index check passes)
    /// but does not match the real on-disk file content. Confirmed
    /// against the real compiled `ValidateOutput` on this exact
    /// scenario (including the `expected A/B, found C/D` message
    /// format).
    #[test]
    fn validate_output_reports_a_real_per_file_hash_mismatch() {
        let (stage, mut assets) = materialized_fixture();
        assets[0].hash = "deadbeef".repeat(8);
        assets[0].size = 999;
        fs::write(
            stage.path().join("overlay-checksums.txt"),
            checksums(&assets),
        )
        .unwrap();
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        let (real_size, real_hash) = hash::hash(&stage.path().join("System/Core.u")).unwrap();
        assert_eq!(
            err,
            format!(
                "output hash mismatch for System/Core.u: expected 999/{}, found {}/{}",
                "deadbeef".repeat(8),
                real_size,
                real_hash
            )
        );
    }

    #[test]
    fn validate_output_reports_an_out_of_date_manifest() {
        let (stage, assets) = materialized_fixture();
        let err = validate_output(stage.path(), &assets, "DIFFERENT\n").unwrap_err();
        assert!(
            err.starts_with("manifest is out of date or non-deterministic: "),
            "{err}"
        );
    }

    #[test]
    fn validate_output_reports_an_out_of_date_checksums_index() {
        let (stage, assets) = materialized_fixture();
        fs::write(stage.path().join("overlay-checksums.txt"), "stale\n").unwrap();
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert!(
            err.starts_with("checksums index is out of date or non-deterministic: "),
            "{err}"
        );
    }

    /// Confirmed against the real compiled `ValidateOutput`: a
    /// case-only rename (two-step, since a single `rename` is a no-op
    /// on a case-insensitive filesystem) produces the exact
    /// "incorrect canonical case" message.
    #[test]
    fn validate_output_reports_incorrect_canonical_case() {
        let (stage, assets) = materialized_fixture();
        let core = stage.path().join("System/Core.u");
        let tmp = stage.path().join("System/.renametmp");
        fs::rename(&core, &tmp).unwrap();
        fs::rename(&tmp, stage.path().join("System/core.u")).unwrap();
        let mut checksums_assets = assets.clone();
        checksums_assets[0].path = "System/core.u".to_string();
        fs::write(
            stage.path().join("overlay-checksums.txt"),
            checksums(&checksums_assets),
        )
        .unwrap();
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert_eq!(
            err,
            "output has incorrect canonical case: System/core.u, expected System/Core.u"
        );
    }

    #[test]
    #[cfg(unix)]
    fn validate_output_rejects_a_symlink_anywhere_in_the_tree() {
        let (stage, assets) = materialized_fixture();
        std::os::unix::fs::symlink("/etc/hosts", stage.path().join("System/evil_link")).unwrap();
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert!(
            err.starts_with("symbolic link in output (dangling links are forbidden): "),
            "{err}"
        );
    }

    #[test]
    fn validate_output_rejects_a_prohibited_payload_anywhere_in_the_tree() {
        let (stage, assets) = materialized_fixture();
        write(stage.path(), "System/evil.exe", b"evil");
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert_eq!(err, "prohibited output payload: System/evil.exe");
    }

    #[test]
    fn validate_output_rejects_a_windows_pe_payload_disguised_by_extension() {
        let (stage, assets) = materialized_fixture();
        write(stage.path(), "System/disguised.bin", b"MZ\x90\x00");
        let err = validate_output(stage.path(), &assets, "MANIFEST\n").unwrap_err();
        assert_eq!(
            err,
            "Windows PE payload disguised in output: System/disguised.bin"
        );
    }

    // -- commit_stage -----------------------------------------------------

    #[test]
    fn commit_stage_installs_fresh_when_output_does_not_exist_yet() {
        let parent = tempdir();
        let stage = parent.path().join("stage");
        fs::create_dir(&stage).unwrap();
        write(&stage, "file.txt", b"staged");
        let output = parent.path().join("out");
        commit_stage(&stage, &output).unwrap();
        assert_eq!(fs::read(output.join("file.txt")).unwrap(), b"staged");
        assert!(!stage.exists(), "stage should be consumed by the rename");
    }

    #[test]
    fn commit_stage_replaces_existing_output_atomically_with_no_backup_leftover() {
        let parent = tempdir();
        let stage = parent.path().join("stage");
        fs::create_dir(&stage).unwrap();
        write(&stage, "file.txt", b"new");
        let output = parent.path().join("out");
        fs::create_dir(&output).unwrap();
        write(&output, "file.txt", b"old");
        commit_stage(&stage, &output).unwrap();
        assert_eq!(fs::read(output.join("file.txt")).unwrap(), b"new");
        let leftovers: Vec<_> = fs::read_dir(parent.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".old-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn commit_stage_refuses_to_replace_a_non_directory_output() {
        let parent = tempdir();
        let stage = parent.path().join("stage");
        fs::create_dir(&stage).unwrap();
        let output = parent.path().join("out");
        write(parent.path(), "out", b"a file, not a directory");
        let err = commit_stage(&stage, &output).unwrap_err();
        assert!(
            err.starts_with("refusing to replace non-directory output: "),
            "{err}"
        );
    }

    /// Confirmed against the real compiled `CommitStage`: when the
    /// rename of `stage` into place fails *after* the existing output
    /// was already backed up (simulated here, portably and
    /// deterministically, by pointing `stage` at a path that does not
    /// exist -- any post-backup rename failure takes the same recovery
    /// path), the original output content is restored and no backup
    /// directory is left behind.
    #[test]
    fn commit_stage_restores_the_original_output_if_the_final_rename_fails() {
        let parent = tempdir();
        let missing_stage = parent.path().join("does-not-exist");
        let output = parent.path().join("out");
        fs::create_dir(&output).unwrap();
        write(&output, "file.txt", b"original content");
        let err = commit_stage(&missing_stage, &output).unwrap_err();
        assert!(err.starts_with("cannot atomically replace "), "{err}");
        assert_eq!(
            fs::read(output.join("file.txt")).unwrap(),
            b"original content"
        );
        let leftovers: Vec<_> = fs::read_dir(parent.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".old-"))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    fn default_options() -> Options {
        Options {
            retail_root: String::new(),
            archive: String::new(),
            prototype_root: String::new(),
            output: String::new(),
            profile: Profile::Safe,
            link_mode: LinkMode::Auto,
            check_only: false,
        }
    }

    #[test]
    fn run_import_requires_exactly_one_of_retail_root_or_archive() {
        let mut options = default_options();
        options.output = "/tmp/wherever".to_string();
        options.prototype_root = "/tmp/wherever".to_string();
        let err = run_import(&options, None).unwrap_err();
        assert_eq!(
            err,
            "exactly one of --retail-root and --archive is required"
        );
        options.retail_root = "/a".to_string();
        options.archive = "/b".to_string();
        let err = run_import(&options, None).unwrap_err();
        assert_eq!(
            err,
            "exactly one of --retail-root and --archive is required"
        );
    }

    #[test]
    fn run_import_requires_output() {
        let mut options = default_options();
        options.retail_root = "/a".to_string();
        options.prototype_root = "/b".to_string();
        let err = run_import(&options, None).unwrap_err();
        assert_eq!(err, "--output is required");
    }

    #[test]
    fn run_import_requires_prototype_root_for_non_retail_only_profiles() {
        let dir = tempdir();
        let mut options = default_options();
        options.retail_root = dir.path().display().to_string();
        options.output = dir.path().join("out").display().to_string();
        let err = run_import(&options, None).unwrap_err();
        assert_eq!(
            err,
            "--prototype-root is required for safe and full profiles"
        );
    }

    #[test]
    fn run_import_rejects_an_archive_path_that_is_not_a_regular_file() {
        let dir = tempdir();
        let mut options = default_options();
        options.archive = dir.path().join("nonexistent.zip").display().to_string();
        options.prototype_root = dir.path().display().to_string();
        options.output = dir.path().join("out").display().to_string();
        let err = run_import(&options, None).unwrap_err();
        assert!(
            err.starts_with("retail archive is not a regular file: "),
            "{err}"
        );
    }

    #[test]
    fn run_import_rejects_overlapping_output_and_retail_root() {
        let dir = tempdir();
        for sub in ["System", "Maps", "Textures", "Sounds", "Music"] {
            fs::create_dir(dir.path().join(sub)).unwrap();
        }
        let mut options = default_options();
        options.retail_root = dir.path().display().to_string();
        options.profile = Profile::RetailOnly;
        // The output is *inside* the retail root -- a real overlap.
        options.output = dir.path().join("out").display().to_string();
        let err = run_import(&options, None).unwrap_err();
        assert_eq!(err, "output and source trees must not overlap");
    }

    /// Not a happy-path test -- same reasoning as `plan`'s own pipeline
    /// test (satisfying every one of `validate_plan`'s exact count
    /// contracts is disproportionate setup for an environment with no
    /// real data to test against anyway). This proves `run_import`'s
    /// own orchestration layer -- argument validation, output-path
    /// resolution (including creating the output's parent directory for
    /// real), the retail/prototype root checks, the overlap check, and
    /// finally calling `plan` -- all run correctly and the real
    /// `validate_plan` error comes back out through every layer intact,
    /// not swallowed, altered, or replaced by a generic wrapper message.
    #[test]
    fn run_import_runs_the_full_pipeline_and_reaches_validate_plan() {
        let scratch = tempdir();
        let retail = scratch.path().join("retail");
        let prototype = scratch.path().join("prototype");
        for root in [&retail, &prototype] {
            for sub in [
                "System",
                "Maps",
                "Textures",
                "Sounds",
                "Music",
                "System/Cutscenes",
            ] {
                fs::create_dir_all(root.join(sub)).unwrap();
            }
            write(root, "System/Default.ini", b"[Core.System]\n");
            write(root, "System/DefUser.ini", b"");
        }
        write(&retail, "Textures/HP2_Menu.utx", b"menu");
        let mut options = default_options();
        options.retail_root = retail.display().to_string();
        options.prototype_root = prototype.display().to_string();
        options.output = scratch.path().join("out").display().to_string();
        let err = run_import(&options, None).unwrap_err();
        assert_eq!(err, "retail Ogg contract mismatch: expected 141, found 0");
        // Confirms run_import never materialized anything for a plan
        // that failed validation.
        assert!(!scratch.path().join("out").exists());
    }
}
