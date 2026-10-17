//! Replaces the generic (non-fd-relative) POSIX atomic-write primitives
//! from `HP2LauncherStore.cpp`: `Join`/`Parent`/`Base`/`ErrorAt` (the
//! small path/error-text helpers used throughout the file), and
//! `Stage`/`Discard`/`SyncParent`/`Publish`/`RestorePublishedFile`/
//! `Backup`/`Directory`/`Names` -- the POSIX `#else` branch (this
//! project targets macOS/Linux only; the `HP2_WINDOWS` branch is not
//! ported, matching `store.rs`'s own scoping note).
//!
//! **Not in this module:** the `openat`/`O_NOFOLLOW`-based directory-fd
//! traversal security hardening (`OpenDirectoryNoFollow`,
//! `OpenDirectoryAtNoFollow`, `DirectoryNamesAt`, `CopyRegularFileAt`,
//! `CopyLegacyTreeAt`, `RemoveTreeAt`, and friends), save discovery,
//! profile staging/publishing, legacy-tree migration, and the
//! `Load`/`Commit*` orchestration -- deliberately scoped as a later,
//! separate increment; see `store.rs`'s own module doc for the same
//! reasoning applied to this file's pure-logic half.
//!
//! The C++ test-only fault-injection hook (`GFailAfterPublication`/
//! `GPublicationCount`/`SetLauncherPublishFailureForTesting`, gated on
//! `HP2_LAUNCHER_TESTING`) is not ported: it exists purely so the real
//! C++ unit tests can force a `Publish` failure after the rename
//! succeeds, and this crate's own tests use Rust-native fault
//! injection instead (a read-only directory, an existing destination,
//! and so on) where that kind of coverage is needed.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::fd::IntoRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Replaces `Join`.
pub(crate) fn join(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    if b.is_empty() {
        return a.to_string();
    }
    if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// Replaces `Parent`. Matches the real C++ exactly, including its own
/// quirk for a path with no `/` at all: `path.substr(0, std::string::
/// npos)` when no slash is found copies the *entire* input back
/// unchanged, not an empty string -- `Parent("foo")` is `"foo"`, not
/// `""`.
pub(crate) fn parent(path: &str) -> String {
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(slash) => path[..slash].to_string(),
        None => path.to_string(),
    }
}

/// Replaces `Base`.
pub(crate) fn base(path: &str) -> &str {
    match path.rfind('/') {
        Some(slash) => &path[slash + 1..],
        None => path,
    }
}

fn strerror_of(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => hp2_ffi_support::strerror(code),
        None => e.to_string(),
    }
}

/// Replaces the POSIX branch of `ErrorAt`.
fn error_at(operation: &str, path: &str, err: &io::Error) -> String {
    format!("{operation} '{path}': {}", strerror_of(err))
}

/// Replaces `StagedFile`.
#[derive(Default, Debug)]
pub struct StagedFile {
    pub destination: String,
    pub temporary: String,
    pub directory: String,
    pub active: bool,
}

/// A 64-bit value with no determinism/reproducibility requirement (used
/// only to build an unpredictable temporary filename, immediately
/// discarded) -- same `RandomState`-based technique
/// `crate::hp2_import::plan::random_probe_name`/`output::random_hex`
/// already use for the identical reason in that crate, avoiding a
/// `rand` dependency for one non-cryptographic call site. Not required
/// to match the real C++'s own `std::random_device`-sourced nonce bit
/// for bit -- both sides only need "hard to guess," and temp filenames
/// are never compared across implementations by any test.
fn random_nonce() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

/// Replaces the POSIX branch of `Stage`. `mode` is the final
/// permission bits (masked to the low 12 bits, matching `mode & 07777`)
/// applied via `fchmod` after the file is written and before it is
/// synced -- the temporary file itself is always created `0600`
/// regardless of `mode`, matching the real C++ exactly (the looser
/// final mode only takes effect once the content is fully written).
pub fn stage(destination: &str, bytes: &[u8], mode: u32) -> Result<StagedFile, String> {
    let directory = parent(destination);
    let mut opened: Option<fs::File> = None;
    let mut temporary = String::new();
    for _ in 0..128 {
        let nonce = random_nonce();
        temporary = format!("{directory}/.{}.tmp.{nonce:x}", base(destination));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)
        {
            Ok(file) => {
                opened = Some(file);
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(error_at("Unable to create temporary file", &temporary, &e)),
        }
    }
    let Some(mut file) = opened else {
        return Err("Unable to allocate an unpredictable temporary filename.".to_string());
    };
    let mut first_error: Option<io::Error> = None;
    if let Err(e) = file.write_all(bytes) {
        first_error = Some(e);
    }
    if first_error.is_none()
        && let Err(e) = file.set_permissions(fs::Permissions::from_mode(mode & 0o7777))
    {
        first_error = Some(e);
    }
    if first_error.is_none()
        && let Err(e) = file.sync_all()
    {
        first_error = Some(e);
    }
    // Matches the real C++'s explicit `::close(fd)` return-value check
    // (a real, reachable failure mode on some filesystems/NFS setups,
    // not merely a formality): Rust's `File`'s `Drop` silently discards
    // any close() error, so the raw fd is closed explicitly here
    // instead, preserving whichever failure (the write/chmod/fsync
    // above, or close() itself if nothing earlier failed) is reported.
    let fd = file.into_raw_fd();
    if unsafe { libc::close(fd) } != 0 && first_error.is_none() {
        first_error = Some(io::Error::last_os_error());
    }
    if let Some(e) = first_error {
        let message = error_at("Unable to stage", destination, &e);
        let _ = fs::remove_file(&temporary);
        return Err(message);
    }
    Ok(StagedFile {
        destination: destination.to_string(),
        temporary,
        directory,
        active: true,
    })
}

/// Replaces `Discard`.
pub fn discard(staged: &mut StagedFile) {
    if staged.active {
        let _ = fs::remove_file(&staged.temporary);
    }
    staged.active = false;
}

/// Replaces the POSIX branch of `SyncParent`.
pub fn sync_parent(directory: &str) -> Result<(), String> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY)
        .open(directory)
    {
        Ok(f) => f,
        Err(e) => return Err(error_at("Unable to open directory", directory, &e)),
    };
    let mut first_error: Option<io::Error> = None;
    if let Err(e) = file.sync_all() {
        first_error = Some(e);
    }
    let fd = file.into_raw_fd();
    if unsafe { libc::close(fd) } != 0 && first_error.is_none() {
        first_error = Some(io::Error::last_os_error());
    }
    match first_error {
        Some(e) => Err(error_at("Unable to synchronize directory", directory, &e)),
        None => Ok(()),
    }
}

/// Test-only fault injection, replacing `GFailAfterPublication`/
/// `GPublicationCount`/`SetLauncherPublishFailureForTesting`: the
/// real C++ only compiles these under `#ifdef HP2_LAUNCHER_TESTING`
/// (present only in the `hp2_launcher_tests` binary, never
/// `hp2_game`). This crate has no equivalent per-target Cargo
/// feature gating `hp2-ffi`'s single shared staticlib, so the hook is
/// always compiled in instead -- harmless in production: the
/// threshold starts at 0, which `publish` never matches (the count
/// is incremented before comparing, so it is always >= 1), and
/// nothing outside test code ever calls `set_publish_failure_for_testing`
/// to change that. The C++ adapter that will call this crate's FFI
/// still gates its own `SetLauncherPublishFailureForTesting` export
/// behind `#ifdef HP2_LAUNCHER_TESTING`, matching the header exactly.
static G_FAIL_AFTER_PUBLICATION: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(0);
static G_PUBLICATION_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Replaces `SetLauncherPublishFailureForTesting`. No dedicated unit
/// test here deliberately: `G_PUBLICATION_COUNT` is process-wide
/// state, and `cargo test` runs this crate's tests in parallel by
/// default -- any test that set a nonzero threshold could race every
/// other concurrently-running test's own `publish()` calls on the
/// same counter, making the suite flaky. Real verification is the
/// `native_launcher_contract` ctest (`Tests/LauncherTests.cpp`,
/// single-threaded `main()`-style, no such race), which exercises
/// this exact mechanism directly.
pub fn set_publish_failure_for_testing(publication_index: i32) {
    use std::sync::atomic::Ordering;
    G_FAIL_AFTER_PUBLICATION.store(publication_index, Ordering::SeqCst);
    G_PUBLICATION_COUNT.store(0, Ordering::SeqCst);
}

/// Replaces `Publish`.
pub fn publish(staged: &mut StagedFile) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if let Err(e) = fs::rename(&staged.temporary, &staged.destination) {
        let message = error_at("Unable to replace", &staged.destination, &e);
        discard(staged);
        return Err(message);
    }
    staged.active = false;
    let count = G_PUBLICATION_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
    if G_FAIL_AFTER_PUBLICATION.load(Ordering::SeqCst) == count {
        return Err(error_at(
            "Injected failure after replacing",
            &staged.destination,
            &io::Error::from_raw_os_error(libc::EIO),
        ));
    }
    sync_parent(&staged.directory)
}

/// Replaces `RestorePublishedFile`.
///
/// When `existed` is `true` but `original` was never actually staged
/// (`!original.active`), the real C++ still calls `ErrorAt`, reading
/// whatever `errno` happens to hold from some unrelated earlier
/// operation (the rename is never attempted, short-circuited by `||`)
/// -- not a reproducible value, and not one any caller in this
/// codebase should ever hit (a `true` `existed` always comes paired
/// with a `StagedFile` a real `Backup`/`Stage` call just populated).
/// This reports a generic, clearly-synthetic reason in that case
/// instead of trying to simulate stale `errno` state.
pub fn restore_published_file(
    destination: &str,
    existed: bool,
    original: &mut StagedFile,
) -> Result<(), String> {
    if existed {
        let rename_result = if original.active {
            fs::rename(&original.temporary, destination)
        } else {
            Err(io::Error::other(
                "restore requested from a StagedFile that was never staged",
            ))
        };
        if let Err(e) = rename_result {
            return Err(error_at("Unable to restore", destination, &e));
        }
        original.active = false;
    } else {
        match fs::remove_file(destination) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(error_at(
                    "Unable to remove partially published file",
                    destination,
                    &e,
                ));
            }
        }
    }
    sync_parent(&parent(destination))
}

/// Replaces the POSIX branch of `Backup`.
pub fn backup(original: &str, bytes: &[u8], mode: u32) -> Result<(), String> {
    let backup_path = format!("{original}.bak");
    match fs::symlink_metadata(&backup_path) {
        Ok(info) => {
            if info.file_type().is_file() {
                return Ok(());
            }
            return Err(format!(
                "Backup path is not a regular non-symlink file: '{backup_path}'."
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(error_at("Unable to inspect backup", &backup_path, &e)),
    }
    let mut staged = stage(&backup_path, bytes, mode)?;
    match fs::hard_link(&staged.temporary, &backup_path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            discard(&mut staged);
            return match fs::symlink_metadata(&backup_path) {
                Ok(info) if info.file_type().is_file() => Ok(()),
                _ => Err(format!(
                    "Backup path is not a regular non-symlink file: '{backup_path}'."
                )),
            };
        }
        Err(e) => {
            let message = error_at("Unable to publish backup", &backup_path, &e);
            discard(&mut staged);
            return Err(message);
        }
    }
    if let Err(e) = fs::remove_file(&staged.temporary) {
        return Err(error_at(
            "Unable to remove backup temporary file",
            &staged.temporary,
            &e,
        ));
    }
    staged.active = false;
    sync_parent(&staged.directory)
}

/// Replaces the POSIX branch of `Directory`.
pub fn directory(path: &str) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

/// Replaces the POSIX branch of `Names`.
///
/// **Known gap, matching this project's established non-UTF-8-filename
/// caveat elsewhere (see `hp2-import::files`'s own module doc):** the
/// real C++ copies `dirent::d_name` into a `std::string` as raw bytes,
/// with no encoding validation at all; this uses
/// `OsStr::to_string_lossy`, which replaces invalid UTF-8 with U+FFFD.
/// A non-UTF-8 filename under a launcher-managed directory is not
/// something this port has verified C++'s exact byte-for-byte behavior
/// for -- flagged, not silently assumed equivalent.
pub fn names(path: &str, missing_okay: bool) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(e) if missing_okay && e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(error_at("Unable to open directory", path, &e)),
    };
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| error_at("Unable to read directory", path, &e))?;
        // `fs::read_dir` already excludes "." and ".." automatically,
        // unlike raw `readdir`, which the real C++ filters explicitly.
        result.push(entry.file_name().to_string_lossy().into_owned());
    }
    result.sort();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::tempdir;
    use std::os::unix::fs::MetadataExt;

    // --- parent/base: verified against the real C++ `Parent`/`Base`
    // via a throwaway probe. ---

    #[test]
    fn parent_of_simple_path_strips_last_component() {
        assert_eq!(parent("/foo/bar.txt"), "/foo");
    }

    #[test]
    fn parent_of_top_level_path_is_root() {
        assert_eq!(parent("/bar.txt"), "/");
    }

    #[test]
    fn parent_with_no_slash_returns_input_unchanged() {
        // Matches the real C++'s own quirk: `substr(0, npos)` with no
        // slash found copies the whole string back, not "".
        assert_eq!(parent("noSlash"), "noSlash");
    }

    #[test]
    fn base_of_simple_path_is_last_component() {
        assert_eq!(base("/foo/bar.txt"), "bar.txt");
    }

    #[test]
    fn base_with_no_slash_returns_input_unchanged() {
        assert_eq!(base("noSlash"), "noSlash");
    }

    // --- stage/discard/publish: verified against the real C++
    // `Stage`/`Discard`/`Publish` via a throwaway probe exercising
    // real file creation, mode bits, content, and rename-publish. ---

    #[test]
    fn stage_creates_mode_and_content_matching_input() {
        let dir = tempdir();
        let dest = dir.path().join("file.txt");
        let staged = stage(dest.to_str().unwrap(), b"hello world", 0o644).unwrap();
        assert!(staged.active);
        let meta = fs::symlink_metadata(&staged.temporary).unwrap();
        assert_eq!(meta.mode() & 0o7777, 0o644);
        assert_eq!(fs::read(&staged.temporary).unwrap(), b"hello world");
        // Temp file lives beside the destination, not at it.
        assert_ne!(staged.temporary, dest.to_str().unwrap());
        assert!(!dest.exists());
    }

    #[test]
    fn stage_honors_arbitrary_mode_bits_not_just_default() {
        let dir = tempdir();
        let dest = dir.path().join("file.txt");
        let staged = stage(dest.to_str().unwrap(), b"x", 0o640).unwrap();
        let meta = fs::symlink_metadata(&staged.temporary).unwrap();
        assert_eq!(meta.mode() & 0o7777, 0o640);
    }

    #[test]
    fn stage_fails_when_parent_directory_is_missing() {
        let dir = tempdir();
        let dest = dir.path().join("missing").join("file.txt");
        let err = stage(dest.to_str().unwrap(), b"x", 0o644).unwrap_err();
        assert!(
            err.contains("Unable to create temporary file"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn discard_removes_the_temporary_file() {
        let dir = tempdir();
        let dest = dir.path().join("file.txt");
        let mut staged = stage(dest.to_str().unwrap(), b"x", 0o644).unwrap();
        let temp_path = staged.temporary.clone();
        assert!(std::path::Path::new(&temp_path).exists());
        discard(&mut staged);
        assert!(!staged.active);
        assert!(!std::path::Path::new(&temp_path).exists());
    }

    #[test]
    fn discard_on_inactive_staged_file_is_a_no_op() {
        let mut staged = StagedFile::default();
        discard(&mut staged); // must not panic on an empty temporary path
        assert!(!staged.active);
    }

    #[test]
    fn publish_moves_staged_content_to_destination() {
        let dir = tempdir();
        let dest = dir.path().join("file.txt");
        let mut staged = stage(dest.to_str().unwrap(), b"content", 0o644).unwrap();
        publish(&mut staged).unwrap();
        assert!(!staged.active);
        assert_eq!(fs::read(&dest).unwrap(), b"content");
    }

    #[test]
    fn publish_atomically_replaces_an_existing_destination() {
        let dir = tempdir();
        let dest = dir.path().join("file.txt");
        fs::write(&dest, b"old content").unwrap();
        let mut staged = stage(dest.to_str().unwrap(), b"new content", 0o644).unwrap();
        publish(&mut staged).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new content");
    }

    // --- backup: verified against the real C++ `Backup`, including
    // the idempotent-already-backed-up path, via a throwaway probe. ---

    #[test]
    fn backup_creates_bak_file_with_given_content() {
        let dir = tempdir();
        let original = dir.path().join("orig.txt");
        backup(original.to_str().unwrap(), b"backup content", 0o644).unwrap();
        let bak = dir.path().join("orig.txt.bak");
        assert_eq!(fs::read(&bak).unwrap(), b"backup content");
    }

    #[test]
    fn backup_is_idempotent_and_keeps_the_first_content() {
        let dir = tempdir();
        let original = dir.path().join("orig.txt");
        backup(original.to_str().unwrap(), b"first", 0o644).unwrap();
        // A second call with different bytes must not overwrite the
        // existing backup -- `Backup` only ever creates one.
        backup(original.to_str().unwrap(), b"second", 0o644).unwrap();
        let bak = dir.path().join("orig.txt.bak");
        assert_eq!(fs::read(&bak).unwrap(), b"first");
    }

    #[test]
    fn backup_rejects_a_non_regular_existing_bak_path() {
        let dir = tempdir();
        let original = dir.path().join("orig.txt");
        let bak = dir.path().join("orig.txt.bak");
        fs::create_dir(&bak).unwrap(); // a directory, not a regular file
        let err = backup(original.to_str().unwrap(), b"x", 0o644).unwrap_err();
        assert!(
            err.contains("is not a regular non-symlink file"),
            "unexpected error: {err}"
        );
    }

    // --- directory/names: verified against the real C++
    // `Directory`/`Names` via a throwaway probe, including the
    // missing-directory `missingOkay` short-circuit and alphabetic
    // sort order. ---

    #[test]
    fn directory_true_for_a_directory_false_for_a_file_or_missing_path() {
        let dir = tempdir();
        let file = dir.path().join("file.txt");
        fs::write(&file, b"x").unwrap();
        assert!(directory(dir.path().to_str().unwrap()));
        assert!(!directory(file.to_str().unwrap()));
        assert!(!directory(dir.path().join("missing").to_str().unwrap()));
    }

    #[test]
    fn names_lists_entries_sorted_excluding_dot_and_dotdot() {
        let dir = tempdir();
        for name in ["z.txt", "a.txt", "m.txt"] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        let result = names(dir.path().to_str().unwrap(), false).unwrap();
        assert_eq!(result, vec!["a.txt", "m.txt", "z.txt"]);
    }

    #[test]
    fn names_on_missing_directory_fails_without_missing_okay() {
        let dir = tempdir();
        let missing = dir.path().join("missing");
        let err = names(missing.to_str().unwrap(), false).unwrap_err();
        assert!(
            err.contains("Unable to open directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn names_on_missing_directory_with_missing_okay_returns_empty() {
        let dir = tempdir();
        let missing = dir.path().join("missing");
        let result = names(missing.to_str().unwrap(), true).unwrap();
        assert!(result.is_empty());
    }

    // --- restore_published_file: verified against the real C++
    // `RestorePublishedFile` for both the `existed` and
    // `!existed` branches via a throwaway probe. ---

    #[test]
    fn restore_published_file_renames_original_temp_back_over_destination() {
        let dir = tempdir();
        let dest = dir.path().join("target.txt");
        let temp = dir.path().join(".restore_temp");
        fs::write(&dest, b"current").unwrap();
        fs::write(&temp, b"original").unwrap();
        let mut original = StagedFile {
            destination: String::new(),
            temporary: temp.to_str().unwrap().to_string(),
            directory: String::new(),
            active: true,
        };
        restore_published_file(dest.to_str().unwrap(), true, &mut original).unwrap();
        assert!(!original.active);
        assert_eq!(fs::read(&dest).unwrap(), b"original");
    }

    #[test]
    fn restore_published_file_removes_destination_when_it_did_not_previously_exist() {
        let dir = tempdir();
        let dest = dir.path().join("partial.txt");
        fs::write(&dest, b"partial").unwrap();
        let mut original = StagedFile::default();
        restore_published_file(dest.to_str().unwrap(), false, &mut original).unwrap();
        assert!(!dest.exists());
    }

    #[test]
    fn restore_published_file_not_existed_is_fine_when_destination_already_absent() {
        let dir = tempdir();
        let dest = dir.path().join("never_there.txt");
        let mut original = StagedFile::default();
        // Must not fail just because there was nothing to remove
        // (matches the real C++'s `errno != ENOENT` guard).
        restore_published_file(dest.to_str().unwrap(), false, &mut original).unwrap();
    }
}
