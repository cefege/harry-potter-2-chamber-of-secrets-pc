//! Replaces the profile staging/publishing functions from
//! `HP2LauncherStore.cpp` (POSIX branch): `StagedDirectory`,
//! `DiscardStagedDirectory`, `EnsureDirectoryNoFollow`,
//! `StageProfile`, `PublishProfile`, `RemovePublishedProfile`. Builds
//! directly on `tree.rs`'s fd-relative primitives (a profile is
//! staged into a randomly-named sibling directory, populated via
//! `copy_legacy_tree_at`, then atomically published with a single
//! `renameat`) and `atomic.rs`'s `sync_parent`/`parent`/`base` (the
//! non-fd-relative `SyncParent` the real C++ also uses here, not
//! `tree.rs`'s `sync_directory_fd`, for the one call site --
//! `EnsureDirectoryNoFollow` -- that syncs a freshly-created
//! directory's *parent* by path rather than by an already-open fd).

use std::ffi::CString;
use std::fs::{self, DirBuilder};
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::DirBuilderExt;

use crate::atomic::{base, join, parent, sync_parent};
use crate::tree::{
    close_checked, copy_legacy_tree_at, fstatat_no_follow, open_directory_at_no_follow,
    open_directory_no_follow, remove_tree_at, remove_tree_no_follow, sync_directory_fd,
};

fn cstr(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("path contains an interior NUL byte: '{s}'"))
}

fn strerror_of(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => hp2_ffi_support::strerror(code),
        None => e.to_string(),
    }
}

fn error_at(operation: &str, path: &str, err: &io::Error) -> String {
    format!("{operation} '{path}': {}", strerror_of(err))
}

/// A 64-bit value with no determinism/reproducibility requirement,
/// same technique (and same rationale) as `atomic::stage`'s own
/// `random_nonce`.
fn random_nonce() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

/// Replaces `StagedDirectory`.
#[derive(Default, Debug)]
pub struct StagedDirectory {
    pub destination: String,
    pub temporary: String,
    pub parent: String,
    pub active: bool,
}

/// Replaces `DiscardStagedDirectory`.
pub fn discard_staged_directory(staged: &mut StagedDirectory) {
    if staged.active {
        let _ = remove_tree_no_follow(&staged.temporary);
    }
    staged.active = false;
}

/// Replaces `EnsureDirectoryNoFollow`.
pub fn ensure_directory_no_follow(path: &str) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(info) if info.is_dir() => return Ok(()),
        Ok(_) => return Err(format!("Expected a directory at '{path}'.")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(error_at("Unable to inspect directory", path, &e)),
    }
    if let Err(e) = DirBuilder::new().mode(0o700).create(path) {
        return Err(error_at("Unable to create directory", path, &e));
    }
    sync_parent(&parent(path))
}

const LEGACY_ITEMS: &[&str] = &["Game.ini", "User.ini", "Save", "Cache"];

/// Replaces `StageProfile`.
pub fn stage_profile(
    launcher_root: &str,
    profile_root: &str,
    copy_legacy: bool,
) -> Result<StagedDirectory, String> {
    let parent_dir = parent(profile_root);
    let (parent_fd, _parent_mode) = open_directory_no_follow(&parent_dir)?;

    let temporary_name_prefix = format!(".{}.tmp.", base(profile_root));
    let mut temporary = String::new();
    let mut created = false;
    for _ in 0..128 {
        let nonce = random_nonce();
        let name = format!("{temporary_name_prefix}{nonce:x}");
        temporary = join(&parent_dir, &name);
        let c_name = match cstr(&name) {
            Ok(c) => c,
            Err(e) => {
                drop(parent_fd);
                return Err(e);
            }
        };
        if unsafe { libc::mkdirat(parent_fd.as_raw_fd(), c_name.as_ptr(), 0o700) } == 0 {
            created = true;
            break;
        }
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::AlreadyExists {
            drop(parent_fd);
            return Err(error_at(
                "Unable to create staged profile",
                &temporary,
                &err,
            ));
        }
    }
    if !created {
        drop(parent_fd);
        return Err("Unable to allocate an unpredictable staged profile directory.".to_string());
    }
    let mut staged = StagedDirectory {
        destination: profile_root.to_string(),
        temporary: temporary.clone(),
        parent: parent_dir.clone(),
        active: true,
    };

    let staged_fd =
        match open_directory_at_no_follow(parent_fd.as_raw_fd(), base(&temporary), &temporary) {
            Ok((fd, _mode)) => fd,
            Err(e) => {
                drop(parent_fd);
                discard_staged_directory(&mut staged);
                return Err(e);
            }
        };

    let mut result: Result<(), String> = Ok(());
    let mut launcher_fd: Option<OwnedFd> = None;
    if copy_legacy {
        match open_directory_no_follow(launcher_root) {
            Ok((fd, _mode)) => launcher_fd = Some(fd),
            Err(e) => result = Err(e),
        }
        if result.is_ok() {
            let fd = launcher_fd.as_ref().unwrap().as_raw_fd();
            'items: for name in LEGACY_ITEMS {
                let c_name = match cstr(name) {
                    Ok(c) => c,
                    Err(e) => {
                        result = Err(e);
                        break 'items;
                    }
                };
                match fstatat_no_follow(fd, &c_name) {
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                    Err(e) => {
                        result = Err(error_at(
                            "Unable to inspect legacy profile item",
                            &join(launcher_root, name),
                            &e,
                        ));
                        break 'items;
                    }
                }
                if let Err(e) = copy_legacy_tree_at(
                    fd,
                    name,
                    staged_fd.as_raw_fd(),
                    name,
                    &join(launcher_root, name),
                    &join(&temporary, name),
                ) {
                    result = Err(e);
                    break 'items;
                }
            }
        }
    }
    if let Some(fd) = launcher_fd
        && let Err(e) = close_checked(fd)
        && result.is_ok()
    {
        result = Err(error_at("Unable to close directory", launcher_root, &e));
    }
    if result.is_ok() {
        result = sync_directory_fd(staged_fd.as_raw_fd(), &temporary);
    }
    if let Err(e) = close_checked(staged_fd)
        && result.is_ok()
    {
        result = Err(error_at("Unable to close directory", &temporary, &e));
    }
    if let Err(e) = close_checked(parent_fd)
        && result.is_ok()
    {
        result = Err(error_at("Unable to close directory", &parent_dir, &e));
    }
    match result {
        Ok(()) => Ok(staged),
        Err(e) => {
            discard_staged_directory(&mut staged);
            Err(e)
        }
    }
}

/// Replaces `PublishProfile`.
pub fn publish_profile(staged: &mut StagedDirectory) -> Result<(), String> {
    let (parent_fd, _mode) = match open_directory_no_follow(&staged.parent) {
        Ok(v) => v,
        Err(e) => {
            discard_staged_directory(staged);
            return Err(e);
        }
    };
    let c_destination_base = match cstr(base(&staged.destination)) {
        Ok(c) => c,
        Err(e) => {
            drop(parent_fd);
            discard_staged_directory(staged);
            return Err(e);
        }
    };
    match fstatat_no_follow(parent_fd.as_raw_fd(), &c_destination_base) {
        Ok(_) => {
            drop(parent_fd);
            let message = format!("Profile already exists: '{}'.", staged.destination);
            discard_staged_directory(staged);
            return Err(message);
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            drop(parent_fd);
            let message = error_at("Unable to inspect profile", &staged.destination, &e);
            discard_staged_directory(staged);
            return Err(message);
        }
    }
    let c_temporary_base = match cstr(base(&staged.temporary)) {
        Ok(c) => c,
        Err(e) => {
            drop(parent_fd);
            discard_staged_directory(staged);
            return Err(e);
        }
    };
    if unsafe {
        libc::renameat(
            parent_fd.as_raw_fd(),
            c_temporary_base.as_ptr(),
            parent_fd.as_raw_fd(),
            c_destination_base.as_ptr(),
        )
    } != 0
    {
        let err = io::Error::last_os_error();
        drop(parent_fd);
        let message = error_at("Unable to publish profile", &staged.destination, &err);
        discard_staged_directory(staged);
        return Err(message);
    }
    staged.active = false;

    let synchronized = sync_directory_fd(parent_fd.as_raw_fd(), &staged.parent);
    let closed = close_checked(parent_fd);
    let mut error = match (&synchronized, &closed) {
        (Err(e), _) => e.clone(),
        (Ok(()), Err(e)) => error_at("Unable to close directory", &staged.parent, e),
        (Ok(()), Ok(())) => return Ok(()),
    };
    let publication_error = error.clone();
    if let Err(removal_error) = remove_published_profile(&staged.destination) {
        error = format!("{publication_error} Profile rollback failed: {removal_error}");
    }
    Err(error)
}

/// Replaces `RemovePublishedProfile`.
pub fn remove_published_profile(profile_root: &str) -> Result<(), String> {
    let (parent_fd, _mode) = open_directory_no_follow(&parent(profile_root))?;
    let removed = remove_tree_at(parent_fd.as_raw_fd(), base(profile_root), profile_root);
    if removed.is_ok()
        && let Err(e) = sync_directory_fd(parent_fd.as_raw_fd(), &parent(profile_root))
    {
        drop(parent_fd);
        return Err(e);
    }
    match close_checked(parent_fd) {
        Err(e) if removed.is_ok() => Err(error_at(
            "Unable to close directory",
            &parent(profile_root),
            &e,
        )),
        _ => removed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::tempdir;
    use std::fs;
    use std::os::unix::fs::MetadataExt;

    fn write(dir: &std::path::Path, rel: &str, content: &[u8]) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn build_launcher_root(root: &std::path::Path) {
        write(root, "Game.ini", b"[Game]\nKey=Value\n");
        write(root, "User.ini", b"[User]\n");
        write(root, "Save/Save1.usa", b"save data");
        write(root, "Cache/sub/item.dat", b"cache");
    }

    // --- ensure_directory_no_follow: verified against the real C++
    // `EnsureDirectoryNoFollow` (via the whole-anonymous-namespace-
    // file probe) for a fresh create (mode 0700), an idempotent
    // already-a-directory call, and a path that exists as a file. ---

    #[test]
    fn ensure_directory_no_follow_creates_with_mode_0700() {
        let dir = tempdir();
        let target = dir.path().join("newdir");
        ensure_directory_no_follow(target.to_str().unwrap()).unwrap();
        let meta = fs::symlink_metadata(&target).unwrap();
        assert!(meta.is_dir());
        assert_eq!(meta.mode() & 0o7777, 0o700);
    }

    #[test]
    fn ensure_directory_no_follow_is_idempotent() {
        let dir = tempdir();
        let target = dir.path().join("newdir");
        ensure_directory_no_follow(target.to_str().unwrap()).unwrap();
        ensure_directory_no_follow(target.to_str().unwrap()).unwrap();
    }

    #[test]
    fn ensure_directory_no_follow_rejects_an_existing_file() {
        let dir = tempdir();
        let target = dir.path().join("afile");
        fs::write(&target, b"x").unwrap();
        let err = ensure_directory_no_follow(target.to_str().unwrap()).unwrap_err();
        assert!(
            err.contains("Expected a directory at"),
            "unexpected error: {err}"
        );
    }

    // --- stage_profile/publish_profile/remove_published_profile:
    // verified against the real C++ the same way, performing real
    // filesystem operations: staging with a full four-item legacy
    // copy (Game.ini/User.ini/Save/Cache) then publishing produced a
    // byte-for-byte identical result tree and content to the real
    // C++; publishing onto an already-existing destination failed
    // with the same message and left no leftover staged-temp
    // directory behind (the rollback discard ran); removing a
    // published profile and removing a missing one both matched. ---

    #[test]
    fn stage_then_publish_with_copy_legacy_reproduces_the_full_tree() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        build_launcher_root(&launcher);
        let profiles = dir.path().join("profiles");
        fs::create_dir_all(&profiles).unwrap();
        let profile_root = profiles.join("prof1");

        let mut staged = stage_profile(
            launcher.to_str().unwrap(),
            profile_root.to_str().unwrap(),
            true,
        )
        .unwrap();
        publish_profile(&mut staged).unwrap();
        assert!(!staged.active);

        assert_eq!(
            fs::read(profile_root.join("Game.ini")).unwrap(),
            b"[Game]\nKey=Value\n"
        );
        assert_eq!(
            fs::read(profile_root.join("User.ini")).unwrap(),
            b"[User]\n"
        );
        assert_eq!(
            fs::read(profile_root.join("Save/Save1.usa")).unwrap(),
            b"save data"
        );
        assert_eq!(
            fs::read(profile_root.join("Cache/sub/item.dat")).unwrap(),
            b"cache"
        );
    }

    #[test]
    fn stage_profile_without_copy_legacy_produces_an_empty_tree() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        build_launcher_root(&launcher);
        let profiles = dir.path().join("profiles");
        fs::create_dir_all(&profiles).unwrap();
        let profile_root = profiles.join("prof1");

        let mut staged = stage_profile(
            launcher.to_str().unwrap(),
            profile_root.to_str().unwrap(),
            false,
        )
        .unwrap();
        publish_profile(&mut staged).unwrap();
        let entries: Vec<_> = fs::read_dir(&profile_root).unwrap().collect();
        assert!(entries.is_empty());
    }

    #[test]
    fn publish_onto_an_existing_destination_fails_and_discards_the_staged_temp() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        build_launcher_root(&launcher);
        let profiles = dir.path().join("profiles");
        fs::create_dir_all(&profiles).unwrap();
        let profile_root = profiles.join("prof1");

        let mut first = stage_profile(
            launcher.to_str().unwrap(),
            profile_root.to_str().unwrap(),
            false,
        )
        .unwrap();
        publish_profile(&mut first).unwrap();

        let mut second = stage_profile(
            launcher.to_str().unwrap(),
            profile_root.to_str().unwrap(),
            false,
        )
        .unwrap();
        let temp_path = second.temporary.clone();
        let err = publish_profile(&mut second).unwrap_err();
        assert!(
            err.contains("Profile already exists"),
            "unexpected error: {err}"
        );
        assert!(!second.active);
        // The rollback discard must have removed the staged temp
        // directory, not leaked it.
        assert!(!std::path::Path::new(&temp_path).exists());
        // The remaining directory entries in `profiles` are exactly
        // the originally-published profile -- no leftover temp dirs.
        let names: Vec<_> = fs::read_dir(&profiles)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["prof1".to_string()]);
    }

    #[test]
    fn remove_published_profile_removes_a_populated_tree() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        build_launcher_root(&launcher);
        let profiles = dir.path().join("profiles");
        fs::create_dir_all(&profiles).unwrap();
        let profile_root = profiles.join("prof1");

        let mut staged = stage_profile(
            launcher.to_str().unwrap(),
            profile_root.to_str().unwrap(),
            true,
        )
        .unwrap();
        publish_profile(&mut staged).unwrap();

        remove_published_profile(profile_root.to_str().unwrap()).unwrap();
        assert!(!profile_root.exists());
    }

    #[test]
    fn remove_published_profile_on_a_missing_path_fails() {
        let dir = tempdir();
        fs::create_dir_all(dir.path().join("profiles")).unwrap();
        let missing = dir.path().join("profiles/doesnotexist");
        let err = remove_published_profile(missing.to_str().unwrap()).unwrap_err();
        assert!(
            err.contains("Unable to inspect staged profile item"),
            "unexpected error: {err}"
        );
    }
}
