//! Replaces the `openat`/`O_NOFOLLOW`-based fd-relative directory
//! traversal functions from `HP2LauncherStore.cpp` (`#ifndef
//! HP2_WINDOWS` branch; this project targets macOS/Linux only):
//! `OpenDirectoryNoFollow`, `OpenDirectoryAtNoFollow`,
//! `DirectoryNamesAt`, `SyncDirectoryFd`, `InspectLegacyTreeAt`,
//! `CopyRegularFileAt`, `SynchronizeCopiedDirectory`,
//! `CopyLegacyTreeAt`, `RemoveTreeAt`, `RemoveTreeNoFollow`.
//!
//! This is the security-hardening layer used while staging/migrating
//! a legacy profile tree: every directory is opened with
//! `O_NOFOLLOW`, every traversal step uses an `*at` syscall relative
//! to an already-open, already-verified directory fd (never by
//! re-resolving a path string from the root), so a symlink swapped in
//! partway through a recursive walk can't redirect an open onto an
//! unintended target (a classic TOCTOU race). None of the standard
//! library exposes `openat`/`fstatat`/`mkdirat`/`unlinkat`/`fdopendir`
//! (no safe wrapper exists for fd-relative POSIX operations), so this
//! module is `libc`-based throughout, matching the precedent already
//! set by `hp2-import::archive`'s hand-written libarchive bindings.
//!
//! **Not in this module:** the orchestration that actually calls
//! these (`StageProfile`, `MigrateLegacySettings`, `PublishProfile`,
//! `DiscoverSaves`, the top-level `Load`/`Commit*` functions) --
//! deliberately scoped as a later, separate increment, matching this
//! port's established slicing pattern.

use std::ffi::{CStr, CString};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

use crate::atomic::{base, join, parent};

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn __error() -> *mut libc::c_int;
}
#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn __errno_location() -> *mut libc::c_int;
}

fn errno_ptr() -> *mut libc::c_int {
    #[cfg(target_os = "macos")]
    unsafe {
        __error()
    }
    #[cfg(target_os = "linux")]
    unsafe {
        __errno_location()
    }
}

fn set_errno(value: i32) {
    unsafe { *errno_ptr() = value };
}

fn get_errno() -> i32 {
    unsafe { *errno_ptr() }
}

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

fn is_reg(mode: libc::mode_t) -> bool {
    (mode as libc::c_uint & libc::S_IFMT as libc::c_uint) == libc::S_IFREG as libc::c_uint
}

fn is_dir(mode: libc::mode_t) -> bool {
    (mode as libc::c_uint & libc::S_IFMT as libc::c_uint) == libc::S_IFDIR as libc::c_uint
}

fn fstat(fd: RawFd) -> io::Result<libc::stat> {
    let mut info = MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, info.as_mut_ptr()) } == 0 {
        Ok(unsafe { info.assume_init() })
    } else {
        Err(io::Error::last_os_error())
    }
}

fn fstatat_no_follow(parent_fd: RawFd, name: &CStr) -> io::Result<libc::stat> {
    let mut info = MaybeUninit::<libc::stat>::uninit();
    let result = unsafe {
        libc::fstatat(
            parent_fd,
            name.as_ptr(),
            info.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == 0 {
        Ok(unsafe { info.assume_init() })
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Explicit close with error-checking, matching every `::close(fd)`
/// return-value check the real C++ performs (a real, reachable
/// failure mode on some filesystems, not mere formality) --
/// `OwnedFd`'s own `Drop` silently discards any close() error.
fn close_checked(fd: OwnedFd) -> io::Result<()> {
    let raw = fd.into_raw_fd();
    if unsafe { libc::close(raw) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Replaces `OpenDirectoryNoFollow`.
pub fn open_directory_no_follow(path: &str) -> Result<(OwnedFd, libc::mode_t), String> {
    let c_path = cstr(path)?;
    let raw = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if raw < 0 {
        return Err(error_at(
            "Unable to open directory",
            path,
            &io::Error::last_os_error(),
        ));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    match fstat(fd.as_raw_fd()) {
        Ok(info) if is_dir(info.st_mode) => Ok((fd, info.st_mode & 0o7777)),
        _ => Err(format!("Unable to verify directory '{path}'.")),
    }
}

/// Replaces `OpenDirectoryAtNoFollow`.
pub fn open_directory_at_no_follow(
    parent_fd: RawFd,
    name: &str,
    display_path: &str,
) -> Result<(OwnedFd, libc::mode_t), String> {
    let c_name = cstr(name)?;
    let raw = unsafe {
        libc::openat(
            parent_fd,
            c_name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if raw < 0 {
        return Err(error_at(
            "Unable to open directory",
            display_path,
            &io::Error::last_os_error(),
        ));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    match fstat(fd.as_raw_fd()) {
        Ok(info) if is_dir(info.st_mode) => Ok((fd, info.st_mode & 0o7777)),
        _ => Err(format!("Unable to verify directory '{display_path}'.")),
    }
}

/// Replaces `DirectoryNamesAt`.
///
/// Same non-UTF-8-filename caveat as `atomic::names`: `d_name` is
/// decoded via `to_string_lossy`, not verified byte-for-byte against
/// the real C++'s raw-byte `std::string` copy.
pub fn directory_names_at(fd: RawFd, display_path: &str) -> Result<Vec<String>, String> {
    let copy = unsafe { libc::dup(fd) };
    if copy < 0 {
        return Err(error_at(
            "Unable to duplicate directory descriptor",
            display_path,
            &io::Error::last_os_error(),
        ));
    }
    let dir = unsafe { libc::fdopendir(copy) };
    if dir.is_null() {
        let err = io::Error::last_os_error();
        unsafe { libc::close(copy) };
        return Err(error_at("Unable to open directory", display_path, &err));
    }
    let mut names = Vec::new();
    set_errno(0);
    loop {
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if name != "." && name != ".." {
            names.push(name);
        }
        set_errno(0);
    }
    let read_error = get_errno();
    let close_result = unsafe { libc::closedir(dir) };
    if close_result != 0 && read_error == 0 {
        return Err(error_at(
            "Unable to close directory",
            display_path,
            &io::Error::last_os_error(),
        ));
    }
    if read_error != 0 {
        return Err(error_at(
            "Unable to read directory",
            display_path,
            &io::Error::from_raw_os_error(read_error),
        ));
    }
    names.sort();
    Ok(names)
}

/// Replaces `SyncDirectoryFd`.
pub fn sync_directory_fd(fd: RawFd, path: &str) -> Result<(), String> {
    if unsafe { libc::fsync(fd) } == 0 {
        Ok(())
    } else {
        Err(error_at(
            "Unable to synchronize directory",
            path,
            &io::Error::last_os_error(),
        ))
    }
}

/// Replaces `InspectLegacyTreeAt`.
pub fn inspect_legacy_tree_at(
    parent_fd: RawFd,
    name: &str,
    path: &str,
    missing_okay: bool,
) -> Result<(), String> {
    let c_name = cstr(name)?;
    let info = match fstatat_no_follow(parent_fd, &c_name) {
        Ok(info) => info,
        Err(e) if missing_okay && e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(error_at("Unable to inspect legacy profile item", path, &e)),
    };
    if is_reg(info.st_mode) {
        return Ok(());
    }
    if !is_dir(info.st_mode) {
        return Err(format!(
            "Legacy profile item is not a regular non-symlink file or directory: '{path}'."
        ));
    }
    let (directory_fd, _mode) = open_directory_at_no_follow(parent_fd, name, path)?;
    let mut result = directory_names_at(directory_fd.as_raw_fd(), path).and_then(|names| {
        for child in &names {
            inspect_legacy_tree_at(directory_fd.as_raw_fd(), child, &join(path, child), false)?;
        }
        Ok(())
    });
    if let Err(e) = close_checked(directory_fd)
        && result.is_ok()
    {
        result = Err(error_at("Unable to close directory", path, &e));
    }
    result
}

/// Replaces `CopyRegularFileAt`.
pub fn copy_regular_file_at(
    source_parent_fd: RawFd,
    source_name: &str,
    destination_parent_fd: RawFd,
    destination_name: &str,
    source_path: &str,
    destination_path: &str,
) -> Result<(), String> {
    let c_source_name = cstr(source_name)?;
    let c_dest_name = cstr(destination_name)?;
    let raw_source = unsafe {
        libc::openat(
            source_parent_fd,
            c_source_name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW,
        )
    };
    if raw_source < 0 {
        return Err(error_at(
            "Unable to open legacy profile file",
            source_path,
            &io::Error::last_os_error(),
        ));
    }
    let source_fd = unsafe { OwnedFd::from_raw_fd(raw_source) };
    let source_info = match fstat(source_fd.as_raw_fd()) {
        Ok(info) if is_reg(info.st_mode) => info,
        // `source_fd` drops here, closing it -- matches the real
        // C++'s own unchecked `::close(sourceFd)` in this branch.
        _ => {
            return Err(format!(
                "Legacy profile item is not a regular non-symlink file: '{source_path}'."
            ));
        }
    };
    let raw_destination = unsafe {
        libc::openat(
            destination_parent_fd,
            c_dest_name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
            0o600_u32,
        )
    };
    if raw_destination < 0 {
        return Err(error_at(
            "Unable to create staged profile file",
            destination_path,
            &io::Error::last_os_error(),
        ));
    }
    let destination_fd = unsafe { OwnedFd::from_raw_fd(raw_destination) };

    let mut first_error: Option<io::Error> = None;
    let mut buffer = [0u8; 32768];
    'copy: loop {
        let count = unsafe {
            libc::read(
                source_fd.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        if count < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            first_error = Some(err);
            break;
        }
        if count == 0 {
            break;
        }
        let mut written = 0usize;
        while written < count as usize {
            let result = unsafe {
                libc::write(
                    destination_fd.as_raw_fd(),
                    buffer[written..count as usize].as_ptr().cast(),
                    count as usize - written,
                )
            };
            if result < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                first_error = Some(err);
                break 'copy;
            }
            if result == 0 {
                first_error = Some(io::Error::from_raw_os_error(libc::EIO));
                break 'copy;
            }
            written += result as usize;
        }
    }
    if first_error.is_none()
        && unsafe {
            libc::fchmod(
                destination_fd.as_raw_fd(),
                (source_info.st_mode & 0o7777) as libc::mode_t,
            )
        } != 0
    {
        first_error = Some(io::Error::last_os_error());
    }
    if first_error.is_none() && unsafe { libc::fsync(destination_fd.as_raw_fd()) } != 0 {
        first_error = Some(io::Error::last_os_error());
    }
    if let Err(e) = close_checked(destination_fd)
        && first_error.is_none()
    {
        first_error = Some(e);
    }
    if let Err(e) = close_checked(source_fd)
        && first_error.is_none()
    {
        first_error = Some(e);
    }
    if let Some(e) = first_error {
        let message = error_at("Unable to copy legacy profile file", source_path, &e);
        let _ = unsafe { libc::unlinkat(destination_parent_fd, c_dest_name.as_ptr(), 0) };
        return Err(message);
    }
    Ok(())
}

/// Replaces `SynchronizeCopiedDirectory`.
pub fn synchronize_copied_directory(
    fd: RawFd,
    mode: libc::mode_t,
    path: &str,
) -> Result<(), String> {
    if unsafe { libc::fchmod(fd, mode & 0o7777) } != 0 {
        return Err(error_at(
            "Unable to set staged profile directory permissions",
            path,
            &io::Error::last_os_error(),
        ));
    }
    sync_directory_fd(fd, path)
}

/// Replaces `CopyLegacyTreeAt`.
///
/// **Known gap, replicating a genuine quirk in the real C++:** when
/// every content-level step succeeds (recursive copy, chmod, fsync)
/// but a trailing `::close(destinationFd)`/`::close(sourceFd)` call
/// itself fails, the real C++ sets its internal `okay` flag to
/// `false` without ever assigning its `error` out-parameter --
/// callers get `false` with whatever `error` held before (typically
/// empty). This returns `Err(String::new())` in that case rather than
/// inventing a descriptive message the original never produced.
pub fn copy_legacy_tree_at(
    source_parent_fd: RawFd,
    source_name: &str,
    destination_parent_fd: RawFd,
    destination_name: &str,
    source_path: &str,
    destination_path: &str,
) -> Result<(), String> {
    let c_source_name = cstr(source_name)?;
    let info = match fstatat_no_follow(source_parent_fd, &c_source_name) {
        Ok(info) => info,
        Err(e) => {
            return Err(error_at(
                "Unable to inspect legacy profile item",
                source_path,
                &e,
            ));
        }
    };
    if is_reg(info.st_mode) {
        return copy_regular_file_at(
            source_parent_fd,
            source_name,
            destination_parent_fd,
            destination_name,
            source_path,
            destination_path,
        );
    }
    if !is_dir(info.st_mode) {
        return Err(format!(
            "Legacy profile item is not a regular non-symlink file or directory: '{source_path}'."
        ));
    }
    let c_destination_name = cstr(destination_name)?;
    if unsafe { libc::mkdirat(destination_parent_fd, c_destination_name.as_ptr(), 0o700) } != 0 {
        return Err(error_at(
            "Unable to create staged profile directory",
            destination_path,
            &io::Error::last_os_error(),
        ));
    }
    let (source_fd, source_mode) =
        open_directory_at_no_follow(source_parent_fd, source_name, source_path)?;
    let destination_fd = match open_directory_at_no_follow(
        destination_parent_fd,
        destination_name,
        destination_path,
    ) {
        Ok((fd, _mode)) => fd,
        Err(e) => {
            // Matches the real C++: the already-open source fd is
            // still closed here (result discarded, same as its
            // own unchecked `::close(sourceFd)`), but the
            // destination side was never opened.
            drop(source_fd);
            return Err(e);
        }
    };

    let mut result = directory_names_at(source_fd.as_raw_fd(), source_path).and_then(|names| {
        for child in &names {
            copy_legacy_tree_at(
                source_fd.as_raw_fd(),
                child,
                destination_fd.as_raw_fd(),
                child,
                &join(source_path, child),
                &join(destination_path, child),
            )?;
        }
        Ok(())
    });
    if result.is_ok() {
        result =
            synchronize_copied_directory(destination_fd.as_raw_fd(), source_mode, destination_path);
    }
    if close_checked(destination_fd).is_err() && result.is_ok() {
        result = Err(String::new());
    }
    if close_checked(source_fd).is_err() && result.is_ok() {
        result = Err(String::new());
    }
    result
}

/// Replaces `RemoveTreeAt`.
///
/// Same genuine-C++-quirk caveat as `copy_legacy_tree_at`: a
/// close()-only failure here (the recursive removal itself fully
/// succeeded) reports `Err(String::new())`, matching the real C++
/// never assigning its `error` out-parameter in that path.
pub fn remove_tree_at(parent_fd: RawFd, name: &str, path: &str) -> Result<(), String> {
    let c_name = cstr(name)?;
    let info = match fstatat_no_follow(parent_fd, &c_name) {
        Ok(info) => info,
        Err(e) => return Err(error_at("Unable to inspect staged profile item", path, &e)),
    };
    if is_reg(info.st_mode) {
        return if unsafe { libc::unlinkat(parent_fd, c_name.as_ptr(), 0) } == 0 {
            Ok(())
        } else {
            Err(error_at(
                "Unable to remove staged profile file",
                path,
                &io::Error::last_os_error(),
            ))
        };
    }
    if !is_dir(info.st_mode) {
        return Err(format!(
            "Staged profile contains a symlink or irregular file: '{path}'."
        ));
    }
    let (directory_fd, _mode) = open_directory_at_no_follow(parent_fd, name, path)?;
    let mut result = directory_names_at(directory_fd.as_raw_fd(), path).and_then(|names| {
        for child in &names {
            remove_tree_at(directory_fd.as_raw_fd(), child, &join(path, child))?;
        }
        Ok(())
    });
    if close_checked(directory_fd).is_err() && result.is_ok() {
        result = Err(String::new());
    }
    result?;
    if unsafe { libc::unlinkat(parent_fd, c_name.as_ptr(), libc::AT_REMOVEDIR) } == 0 {
        Ok(())
    } else {
        Err(error_at(
            "Unable to remove staged profile directory",
            path,
            &io::Error::last_os_error(),
        ))
    }
}

/// Replaces `RemoveTreeNoFollow`.
pub fn remove_tree_no_follow(path: &str) -> Result<(), String> {
    let (parent_fd, _mode) = open_directory_no_follow(&parent(path))?;
    let removed = remove_tree_at(parent_fd.as_raw_fd(), base(path), path);
    match close_checked(parent_fd) {
        Ok(()) => removed,
        Err(e) if removed.is_ok() => Err(error_at("Unable to close directory", &parent(path), &e)),
        Err(_) => removed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::tempdir;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

    fn open_parent(path: &str) -> OwnedFd {
        open_directory_no_follow(&parent(path)).unwrap().0
    }

    // --- copy_legacy_tree_at: verified against the real C++
    // `CopyLegacyTreeAt` (via a whole-anonymous-namespace-file probe,
    // since these are internal-linkage functions) on real nested
    // directory trees, including mode-bit propagation and the
    // mkdirat(0700)-then-fchmod(source_mode) sequence that lets the
    // final destination directory mode end up matching the source's,
    // not the initial 0700. ---

    #[test]
    fn copy_legacy_tree_preserves_nested_structure_content_and_modes() {
        let dir = tempdir();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("sub1")).unwrap();
        fs::write(src.join("file1.txt"), b"file1 content").unwrap();
        fs::write(src.join("sub1/file2.txt"), b"file2 content").unwrap();
        fs::set_permissions(src.join("file1.txt"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::set_permissions(src.join("sub1"), fs::Permissions::from_mode(0o751)).unwrap();

        let dst_parent = dir.path().join("dst_parent");
        fs::create_dir_all(&dst_parent).unwrap();
        let dst = dst_parent.join("dest");

        let src_fd = open_parent(src.to_str().unwrap());
        let dst_fd = open_parent(dst.to_str().unwrap());
        copy_legacy_tree_at(
            src_fd.as_raw_fd(),
            "src",
            dst_fd.as_raw_fd(),
            "dest",
            src.to_str().unwrap(),
            dst.to_str().unwrap(),
        )
        .unwrap();

        assert_eq!(fs::read(dst.join("file1.txt")).unwrap(), b"file1 content");
        assert_eq!(
            fs::read(dst.join("sub1/file2.txt")).unwrap(),
            b"file2 content"
        );
        assert_eq!(
            fs::symlink_metadata(dst.join("file1.txt")).unwrap().mode() & 0o7777,
            0o640
        );
        // The destination directory mode ends up matching the
        // *source* directory's mode (via the trailing
        // `SynchronizeCopiedDirectory` fchmod), not the `mkdirat`
        // call's own initial 0700.
        assert_eq!(
            fs::symlink_metadata(dst.join("sub1")).unwrap().mode() & 0o7777,
            0o751
        );
    }

    #[test]
    fn copy_legacy_tree_rejects_a_symlink_in_the_source() {
        let dir = tempdir();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("regular.txt"), b"x").unwrap();
        symlink("/etc/passwd", src.join("evil_link")).unwrap();

        let dst_parent = dir.path().join("dst_parent");
        fs::create_dir_all(&dst_parent).unwrap();
        let dst = dst_parent.join("dest");

        let src_fd = open_parent(src.to_str().unwrap());
        let dst_fd = open_parent(dst.to_str().unwrap());
        let err = copy_legacy_tree_at(
            src_fd.as_raw_fd(),
            "src",
            dst_fd.as_raw_fd(),
            "dest",
            src.to_str().unwrap(),
            dst.to_str().unwrap(),
        )
        .unwrap_err();
        assert!(
            err.contains("is not a regular non-symlink file or directory"),
            "unexpected error: {err}"
        );
    }

    // --- inspect_legacy_tree_at: verified against the real C++
    // `InspectLegacyTreeAt`, including the `missingOkay` short-circuit
    // and symlink rejection, via the same probe. ---

    #[test]
    fn inspect_legacy_tree_accepts_a_valid_nested_tree() {
        let dir = tempdir();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("sub1")).unwrap();
        fs::write(src.join("file1.txt"), b"x").unwrap();
        fs::write(src.join("sub1/file2.txt"), b"x").unwrap();

        let fd = open_parent(src.to_str().unwrap());
        inspect_legacy_tree_at(fd.as_raw_fd(), "src", src.to_str().unwrap(), false).unwrap();
    }

    #[test]
    fn inspect_legacy_tree_rejects_a_symlink() {
        let dir = tempdir();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        symlink("/etc/passwd", src.join("evil_link")).unwrap();

        let fd = open_parent(src.to_str().unwrap());
        let err = inspect_legacy_tree_at(fd.as_raw_fd(), "src", src.to_str().unwrap(), false)
            .unwrap_err();
        assert!(
            err.contains("is not a regular non-symlink file or directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn inspect_legacy_tree_missing_okay_short_circuits_on_a_missing_path() {
        let dir = tempdir();
        let missing = dir.path().join("missing");
        let fd = open_parent(missing.to_str().unwrap());
        inspect_legacy_tree_at(fd.as_raw_fd(), "missing", missing.to_str().unwrap(), true).unwrap();
    }

    #[test]
    fn inspect_legacy_tree_without_missing_okay_fails_on_a_missing_path() {
        let dir = tempdir();
        let missing = dir.path().join("missing");
        let fd = open_parent(missing.to_str().unwrap());
        let err =
            inspect_legacy_tree_at(fd.as_raw_fd(), "missing", missing.to_str().unwrap(), false)
                .unwrap_err();
        assert!(
            err.contains("Unable to inspect legacy profile item"),
            "unexpected error: {err}"
        );
    }

    // --- remove_tree_no_follow: verified against the real C++
    // `RemoveTreeNoFollow` for both a populated nested tree and a
    // single regular file via the same probe. ---

    #[test]
    fn remove_tree_no_follow_removes_a_populated_nested_tree() {
        let dir = tempdir();
        let target = dir.path().join("dest");
        fs::create_dir_all(target.join("sub1")).unwrap();
        fs::write(target.join("file1.txt"), b"x").unwrap();
        fs::write(target.join("sub1/file2.txt"), b"x").unwrap();

        remove_tree_no_follow(target.to_str().unwrap()).unwrap();
        assert!(!target.exists());
    }

    #[test]
    fn remove_tree_no_follow_removes_a_single_regular_file() {
        let dir = tempdir();
        let target = dir.path().join("file.txt");
        fs::write(&target, b"x").unwrap();

        remove_tree_no_follow(target.to_str().unwrap()).unwrap();
        assert!(!target.exists());
    }

    // --- directory_names_at: a lower-level unit not already fully
    // exercised indirectly above (sorted order, dot/dotdot
    // exclusion). ---

    #[test]
    fn directory_names_at_lists_entries_sorted_excluding_dot_and_dotdot() {
        let dir = tempdir();
        for name in ["z.txt", "a.txt", "m.txt"] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        let (fd, _mode) = open_directory_no_follow(dir.path().to_str().unwrap()).unwrap();
        let names = directory_names_at(fd.as_raw_fd(), dir.path().to_str().unwrap()).unwrap();
        assert_eq!(names, vec!["a.txt", "m.txt", "z.txt"]);
    }
}
