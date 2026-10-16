//! Replaces `HP2ImportInputs.cpp`'s `DeterministicFiles`: a deterministic,
//! symlink-rejecting recursive file listing used as the first step of
//! every import source scan.
//!
//! The walk uses an explicit stack, sorting each directory's entries by
//! `(Lower(name), name)` before pushing (reversed, so popping gives sorted
//! order) -- a pre-order DFS by casefolded name. The final file list is
//! then fully re-sorted by `(Lower(relative_path), relative_path)`, which
//! alone determines the returned order regardless of walk order. The
//! per-directory sort during the walk still matters on an error path: if a
//! tree has more than one symlink/unsupported entry, which one `Fail`s
//! first (and so which exact message text comes out) depends on visiting
//! order, which `importer_parity`'s stderr-line comparison can observe.
//! Matching the two-level sort exactly, not just the final output, keeps
//! that error-message text byte-identical to the C++ this replaces.
//!
//! Known gap, matching [`crate::lower`]'s note: this takes `&Path` and
//! requires every path component to be valid UTF-8 (`Path::to_str`),
//! whereas C++'s `Utf8()` goes through `fs::path::u8string()`, which does
//! not reject a non-UTF-8 native encoding the same way. A real-world
//! non-UTF-8 filename under the data root is not something this crate has
//! verified C++'s exact behavior for yet; flagged, not solved here.

use std::fs;
use std::path::{Path, PathBuf};

use crate::lower::lower;

fn fail(message: impl Into<String>) -> String {
    message.into()
}

/// Matches `Utf8(path)`. `None` (mapped to an `Err` by the caller) stands
/// in for the C++ side's implementation-defined handling of a native path
/// that is not valid UTF-8 -- see the module doc.
fn utf8(p: &Path) -> Result<String, String> {
    p.to_str()
        .map(str::to_string)
        .ok_or_else(|| fail(format!("non-UTF-8 path: {}", p.to_string_lossy())))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Dir,
    File,
}

/// Matches `fs::symlink_status` + the three-way `is_symlink`/`is_directory`/
/// `is_regular_file` dispatch: symlinks and anything else (FIFOs, device
/// files, sockets) are rejected, not silently skipped.
fn classify(path: &Path) -> Result<Kind, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| {
        fail(format!(
            "cannot inspect {}: {}",
            path.display(),
            strerror_of(&e)
        ))
    })?;
    let ty = meta.file_type();
    if ty.is_symlink() {
        return Err(fail(format!(
            "symbolic link in input tree: {}",
            utf8(path)?
        )));
    }
    if ty.is_dir() {
        return Ok(Kind::Dir);
    }
    if ty.is_file() {
        return Ok(Kind::File);
    }
    Err(fail(format!("unsupported input entry: {}", utf8(path)?)))
}

fn strerror_of(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => hp2_ffi_support::strerror(code),
        None => e.to_string(),
    }
}

/// Matches the ordering [`folded_then_exact`] implements: casefold first,
/// then the original string as an exact tiebreak. Only used directly by
/// tests that exercise the comparator on plain strings; production sort
/// sites precompute the fold once per entry and call
/// [`folded_then_exact`] instead.
#[cfg(test)]
fn casefold_then_exact(a: &str, b: &str) -> std::cmp::Ordering {
    folded_then_exact(&lower(a), a, &lower(b), b)
}

/// Same ordering as [`casefold_then_exact`], taking an already-lowered
/// key for each side so a sort over many entries lowers each name once
/// instead of twice per comparison.
fn folded_then_exact(fa: &str, xa: &str, fb: &str, xb: &str) -> std::cmp::Ordering {
    if fa == fb { xa.cmp(xb) } else { fa.cmp(fb) }
}

/// Replaces `DeterministicFiles`. `root` must itself be a readable
/// directory (not validated here, matching the C++ function, which lets
/// the first `directory_iterator` construction report that).
pub fn deterministic_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(current) = pending.pop() {
        let mut entries: Vec<PathBuf> = Vec::new();
        let read = fs::read_dir(&current).map_err(|e| {
            fail(format!(
                "cannot inspect {}: {}",
                utf8(&current).unwrap_or_else(|_| current.display().to_string()),
                strerror_of(&e)
            ))
        })?;
        for entry in read {
            let entry = entry.map_err(|e| {
                fail(format!(
                    "cannot inspect {}: {}",
                    utf8(&current).unwrap_or_else(|_| current.display().to_string()),
                    strerror_of(&e)
                ))
            })?;
            entries.push(entry.path());
        }
        // Precompute (casefold key, exact name) before sorting, with `?`
        // propagating a non-UTF-8 name immediately -- matching Lower()'s
        // own Fail on invalid UTF-8, rather than letting a name that
        // can't be read fall back to "" inside the comparator and sort
        // silently (and surface, if at all, as a different, later error).
        let mut keyed: Vec<(String, String, PathBuf)> = Vec::with_capacity(entries.len());
        for entry in entries {
            let name = utf8(Path::new(entry.file_name().unwrap_or_default()))?;
            let folded = lower(&name);
            keyed.push((folded, name, entry));
        }
        keyed.sort_by(|(fa, xa, _), (fb, xb, _)| folded_then_exact(fa, xa, fb, xb));
        for (_, _, entry) in keyed.into_iter().rev() {
            match classify(&entry)? {
                Kind::Dir => pending.push(entry),
                Kind::File => files.push(entry),
            }
        }
    }
    let mut keyed: Vec<(String, String, PathBuf)> = Vec::with_capacity(files.len());
    for f in files {
        let rel = f.strip_prefix(root).unwrap_or(&f).to_path_buf();
        let rel_str = utf8(&rel)?;
        let folded = lower(&rel_str);
        keyed.push((folded, rel_str, f));
    }
    keyed.sort_by(|(fa, xa, _), (fb, xb, _)| folded_then_exact(fa, xa, fb, xb));
    Ok(keyed.into_iter().map(|(_, _, f)| f).collect())
}

/// Replaces `Child(d, w, required)`: the single case-insensitively
/// matching direct child of `d` named `w`. `Ok(None)` only when
/// `required` is `false` and no match exists (matching the C++ side's
/// `fs::path{}` sentinel, which every call site only ever tests with
/// `.empty()`).
pub fn child(dir: &Path, want: &str, required: bool) -> Result<Option<PathBuf>, String> {
    let wanted = lower(want);
    let mut matches: Vec<PathBuf> = Vec::new();
    let read = fs::read_dir(dir).map_err(|e| {
        fail(format!(
            "cannot inspect {}: {}",
            utf8(dir).unwrap_or_else(|_| dir.display().to_string()),
            strerror_of(&e)
        ))
    })?;
    for entry in read {
        let entry = entry.map_err(|e| {
            fail(format!(
                "cannot inspect {}: {}",
                utf8(dir).unwrap_or_else(|_| dir.display().to_string()),
                strerror_of(&e)
            ))
        })?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if lower(name) == wanted {
            matches.push(path);
        }
    }
    if matches.len() > 1 {
        return Err(fail(format!(
            "ambiguous case-insensitive component '{want}' under {}",
            utf8(dir)?
        )));
    }
    let Some(found) = matches.into_iter().next() else {
        if required {
            return Err(fail(format!(
                "missing {want} directory under {}",
                utf8(dir)?
            )));
        }
        return Ok(None);
    };
    let bad = match fs::symlink_metadata(&found) {
        Ok(meta) => meta.file_type().is_symlink() || !meta.file_type().is_dir(),
        Err(_) => true,
    };
    if bad {
        return Err(fail(format!(
            "expected real directory for {want}: {}",
            utf8(&found)?
        )));
    }
    Ok(Some(found))
}

/// Replaces `ChildFile(d, w)`: the single case-insensitively matching
/// direct child of `d` named `w`, required to be a real regular file.
///
/// Known, reachable discrepancy: C++'s `ChildFile` constructs
/// `fs::directory_iterator(d)` without an `error_code` overload, so an
/// unreadable `d` throws `fs::filesystem_error` directly, caught by
/// `RunImport`'s generic `catch (const std::exception&)` branch
/// (`HP2ImportOutput.cpp:273-282`) and surfaced as that exception's
/// (implementation-defined, libstdc++/libc++-specific) `what()` text --
/// not `Fail`'s clean "cannot inspect ..." message `Child` produces for
/// the same failure. This gives the clean message in both cases instead,
/// since replicating an unspecified per-stdlib string isn't practical.
///
/// `Child()` validating a directory before `ChildFile()` reads it does
/// not make this unreachable: `Child` only `symlink_status`-checks the
/// matched entry from its *parent*'s listing (confirming it exists and
/// is a directory), it never reads the directory's own contents. The
/// first read of a directory's contents is whichever of `Child`
/// (recursing into it) or `ChildFile` (listing it directly) touches it
/// next. `CollectRetailRequiredNestedTextures`'s
/// `ChildFile(oldTextures, "HP_Menu.utx")` is exactly that first read of
/// `Textures/Old textures` -- an unreadable (e.g. permission-restricted)
/// `Old textures` on a real retail copy reaches this divergence. Not
/// reproduced here; not verified against a real failing case.
pub fn child_file(dir: &Path, want: &str) -> Result<PathBuf, String> {
    let wanted = lower(want);
    let mut matches: Vec<PathBuf> = Vec::new();
    let read = fs::read_dir(dir).map_err(|e| {
        fail(format!(
            "cannot inspect {}: {}",
            utf8(dir).unwrap_or_else(|_| dir.display().to_string()),
            strerror_of(&e)
        ))
    })?;
    for entry in read {
        let entry = entry.map_err(|e| {
            fail(format!(
                "cannot inspect {}: {}",
                utf8(dir).unwrap_or_else(|_| dir.display().to_string()),
                strerror_of(&e)
            ))
        })?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if lower(name) == wanted {
            matches.push(path);
        }
    }
    if matches.len() != 1 {
        return Err(fail(format!(
            "expected exactly one {want} under {}, found {}",
            utf8(dir)?,
            matches.len()
        )));
    }
    let found = matches.into_iter().next().unwrap();
    let bad = match fs::symlink_metadata(&found) {
        Ok(meta) => meta.file_type().is_symlink() || !meta.file_type().is_file(),
        Err(_) => true,
    };
    if bad {
        return Err(fail(format!(
            "expected real config file: {}",
            utf8(&found)?
        )));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    fn make(root: &Path, rel: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(path).unwrap();
    }

    /// Ground truth hand-verified against a C++ probe built from the exact
    /// `DeterministicFiles`/`Lower`/`Utf8` source this replaces, run over
    /// an identical fixture (names chosen to exercise casefold collisions:
    /// `Straße` vs `strasse 2`, `alpha.TXT` vs `a/`, mixed-case ties).
    #[test]
    fn sorts_by_casefolded_relative_path_then_exact() {
        let dir = tempdir();
        for rel in [
            "alpha.TXT",
            "B/x.txt",
            "Straße/f.txt",
            "a/Y.txt",
            "Zeta.txt",
            "strasse 2/g.txt",
        ] {
            make(dir.path(), rel);
        }
        let files = deterministic_files(dir.path()).unwrap();
        let rels: Vec<String> = files
            .iter()
            .map(|f| utf8(f.strip_prefix(dir.path()).unwrap()).unwrap())
            .collect();
        assert_eq!(
            rels,
            vec![
                "a/Y.txt",
                "alpha.TXT",
                "B/x.txt",
                "strasse 2/g.txt",
                "Straße/f.txt",
                "Zeta.txt",
            ]
        );
    }

    #[test]
    #[cfg(unix)]
    fn rejects_symlinks_with_the_exact_message_text() {
        let dir = tempdir();
        make(dir.path(), "real.txt");
        let link = dir.path().join("alink");
        std::os::unix::fs::symlink("/tmp", &link).unwrap();
        let err = deterministic_files(dir.path()).unwrap_err();
        assert_eq!(
            err,
            format!("symbolic link in input tree: {}", link.to_str().unwrap())
        );
    }

    #[test]
    fn child_finds_a_case_insensitive_match() {
        let dir = tempdir();
        fs::create_dir(dir.path().join("Textures")).unwrap();
        let found = child(dir.path(), "textures", true).unwrap();
        assert_eq!(found, Some(dir.path().join("Textures")));
    }

    // Needs two directory entries that are byte-distinct but collide
    // under Lower()'s full Unicode case folding -- "Sounds" vs "SOUNDS"
    // and a "Straße"/"STRASSE" pair were both tried directly against the
    // real filesystem; both fail to even create a second entry on this
    // repo's primary dev platform (macOS's default case-insensitive APFS
    // folds "ß"/"STRASSE" together too, not just ASCII case), so there is
    // no pair this test can use there. Rather than pass via a silent
    // runtime skip, gate it to run only where it can actually exercise
    // the code path (most Linux ext4 setups, including the linux-arm64
    // CI preset) -- its absence from a macOS test run is the honest
    // signal that this branch isn't covered there, not a pass.
    #[test]
    #[cfg(target_os = "linux")]
    fn child_rejects_ambiguous_case_insensitive_matches() {
        let dir = tempdir();
        fs::create_dir(dir.path().join("Sounds")).unwrap();
        fs::create_dir(dir.path().join("SOUNDS")).unwrap();
        let err = child(dir.path(), "sounds", true).unwrap_err();
        assert!(
            err.starts_with("ambiguous case-insensitive component 'sounds' under "),
            "{err}"
        );
    }

    #[test]
    fn child_optional_returns_none_when_missing_and_errors_when_required() {
        let dir = tempdir();
        assert_eq!(child(dir.path(), "Help", false).unwrap(), None);
        let err = child(dir.path(), "Help", true).unwrap_err();
        assert!(err.starts_with("missing Help directory under "), "{err}");
    }

    #[test]
    fn child_rejects_a_file_with_the_matching_name() {
        let dir = tempdir();
        make(dir.path(), "Music");
        let err = child(dir.path(), "music", true).unwrap_err();
        assert!(
            err.starts_with("expected real directory for music: "),
            "{err}"
        );
    }

    #[test]
    fn child_file_requires_exactly_one_match() {
        let dir = tempdir();
        let err = child_file(dir.path(), "HP_Menu.utx").unwrap_err();
        assert_eq!(
            err,
            format!(
                "expected exactly one HP_Menu.utx under {}, found 0",
                dir.path().to_str().unwrap()
            )
        );
        make(dir.path(), "hp_menu.UTX");
        let found = child_file(dir.path(), "HP_Menu.utx").unwrap();
        assert_eq!(found, dir.path().join("hp_menu.UTX"));
    }

    #[test]
    fn child_file_rejects_a_directory_with_the_matching_name() {
        let dir = tempdir();
        fs::create_dir(dir.path().join("config.ini")).unwrap();
        let err = child_file(dir.path(), "config.ini").unwrap_err();
        assert_eq!(
            err,
            format!(
                "expected real config file: {}",
                dir.path().join("config.ini").to_str().unwrap()
            )
        );
    }

    #[test]
    fn empty_directory_yields_no_files() {
        let dir = tempdir();
        assert_eq!(
            deterministic_files(dir.path()).unwrap(),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn casefold_then_exact_breaks_ties_by_exact_bytes() {
        use std::cmp::Ordering;
        // "File.txt" and "file.TXT" casefold identically; the tiebreak on
        // exact bytes must still give a deterministic order ('F' < 'f').
        assert_eq!(casefold_then_exact("File.txt", "file.TXT"), Ordering::Less);
        assert_eq!(
            casefold_then_exact("file.TXT", "File.txt"),
            Ordering::Greater
        );
        // Different casefold keys order by the fold, not the original
        // bytes: "B" casefolds to "b", "a" stays "a", and "b" > "a".
        assert_eq!(casefold_then_exact("B", "a"), Ordering::Greater);
        // A full-casefold tie (ß -> ss) still breaks on exact bytes, not
        // just an ASCII-range tie.
        assert_eq!(
            casefold_then_exact("Straße", "STRASSE"),
            "Straße".cmp("STRASSE")
        );
        assert_ne!(casefold_then_exact("Straße", "STRASSE"), Ordering::Equal);
        assert_eq!(casefold_then_exact("same", "same"), Ordering::Equal);
    }

    /// Minimal `tempfile`-free scratch directory: unique path under the
    /// OS temp dir, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn tempdir() -> TempDir {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "hp2-import-files-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}
