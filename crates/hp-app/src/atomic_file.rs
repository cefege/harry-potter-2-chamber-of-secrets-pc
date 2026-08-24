//! Shared staged-publication helpers for launcher-owned files
//! (`Stage`/`Publish`/`Backup` from `HP2LauncherStore.cpp`).
//!
//! Contract: originals are backed up once as `<name>.bak` exactly, symlink
//! backup targets are rejected without touching them, new files are created
//! mode 0600, existing modes are retained, and publication is a same-
//! directory staged write followed by an atomic rename.

use std::path::Path;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use crate::error::AppError;

static PUBLICATION_COUNT: AtomicU32 = AtomicU32::new(0);
static FAIL_AFTER_PUBLICATION: AtomicI32 = AtomicI32::new(-1);

/// Test-only fault injection mirroring
/// `SetLauncherPublishFailureForTesting`: the `publication_index`-th
/// [`publish`] call fails right after its rename, exactly like the native
/// oracle hook. Honored only under `HP2_LAUNCHER_TESTING=1`; any other
/// value disables injection.
pub fn set_publish_failure_for_testing(publication_index: i32) {
    if std::env::var("HP2_LAUNCHER_TESTING").is_ok_and(|value| value == "1") {
        FAIL_AFTER_PUBLICATION.store(publication_index, Ordering::SeqCst);
        PUBLICATION_COUNT.store(0, Ordering::SeqCst);
    }
}

/// True exactly once when the injected failure index is reached
/// (`Publish`'s post-rename counter).
fn publication_failure_injected() -> bool {
    let target = FAIL_AFTER_PUBLICATION.load(Ordering::SeqCst);
    if target < 0 {
        return false;
    }
    PUBLICATION_COUNT.fetch_add(1, Ordering::SeqCst) + 1 == target as u32
}

fn io(reason: &'static str, path: &Path, error: std::io::Error) -> AppError {
    AppError::new(
        "app.settings_io_failed",
        format!("{reason} '{}': {error}", path.display()),
    )
}

/// Reads a regular file's bytes and permission bits. Symlinks and other
/// irregular entries are rejected (`app.settings_io_failed`); a missing
/// file yields `None`.
pub fn read_regular(path: &Path) -> Result<Option<(Vec<u8>, u32)>, AppError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io("Unable to inspect", path, error)),
    };
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o7777
    };
    #[cfg(not(unix))]
    let mode = 0o600;
    if !metadata.is_file() {
        return Err(AppError::new(
            "app.settings_io_failed",
            format!("'{}' is not a regular file", path.display()),
        ));
    }
    let bytes = std::fs::read(path).map_err(|error| io("Unable to read", path, error))?;
    Ok(Some((bytes, mode)))
}

/// True when the path exists as a regular directory (no symlink follow).
pub fn is_directory(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

/// Preserves `bytes` as `<destination>.bak` on first use only: an
/// existing regular backup is never overwritten, and a symlinked or
/// irregular backup target aborts without being touched (`Backup`).
pub fn backup(destination: &Path, bytes: &[u8], mode: u32) -> Result<(), AppError> {
    let backup_path = append_extension(destination, ".bak");
    match std::fs::symlink_metadata(&backup_path) {
        Ok(metadata) if metadata.is_file() => return Ok(()),
        Ok(_) => {
            return Err(AppError::new(
                "app.settings_io_failed",
                format!(
                    "Backup path is not a regular non-symlink file: '{}'",
                    backup_path.display()
                ),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io("Unable to inspect backup", &backup_path, error)),
    }
    std::fs::write(&backup_path, bytes).map_err(|error| io("Unable to write backup", &backup_path, error))?;
    set_mode(&backup_path, mode);
    Ok(())
}

/// Restores the exact pre-publication state of `destination`: the
/// `original` bytes when it existed, otherwise removal
/// (`RestorePublishedFile`).
pub fn restore_published(
    destination: &Path,
    existed: bool,
    original: Option<&[u8]>,
    mode: u32,
) -> Result<(), AppError> {
    if !existed {
        return match std::fs::remove_file(destination) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io("Unable to remove published file", destination, error)),
        };
    }
    let bytes = original.ok_or_else(|| {
        AppError::new(
            "app.settings_io_failed",
            format!(
                "Cannot roll back '{}' without its original bytes",
                destination.display()
            ),
        )
    })?;
    let temporary = append_extension(destination, ".rollback.tmp");
    std::fs::write(&temporary, bytes)
        .and_then(|()| std::fs::rename(&temporary, destination))
        .map_err(|error| {
            let _ = std::fs::remove_file(&temporary);
            io("Unable to restore", destination, error)
        })?;
    set_mode(destination, mode);
    Ok(())
}

/// Publishes `bytes` to `destination`. When `existed`, the exact
/// `original` bytes are preserved as `<destination>.bak` on first
/// publication only (later publications never overwrite that backup), and
/// a symlinked backup target aborts the whole operation untouched.
///
/// Publication is a same-directory staged write followed by an atomic
/// rename. An injected test failure (see
/// [`set_publish_failure_for_testing`]) fires after that rename and rolls
/// the destination back before reporting, so callers only ever observe a
/// failed publication whose filesystem effect is invisible.
pub fn publish(
    destination: &Path,
    bytes: &[u8],
    existed: bool,
    original: Option<&[u8]>,
    mode: u32,
) -> Result<(), AppError> {
    if existed {
        let Some(backup_bytes) = original else {
            return Err(AppError::new(
                "app.settings_io_failed",
                format!(
                    "Cannot publish '{}' over an existing file without its original bytes",
                    destination.display()
                ),
            ));
        };
        backup(destination, backup_bytes, mode)?;
    }

    let temporary = append_extension(destination, ".tmp");
    std::fs::write(&temporary, bytes).map_err(|error| io("Unable to stage", &temporary, error))?;
    set_mode(&temporary, mode);
    if let Err(error) = std::fs::rename(&temporary, destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(io("Unable to publish", destination, error));
    }
    if publication_failure_injected() {
        let mut message = format!(
            "Injected failure after replacing '{}'",
            destination.display()
        );
        if let Err(restoration) = restore_published(destination, existed, original, mode) {
            message.push_str(&format!(" Rollback failed: {}", restoration.message()));
        }
        return Err(AppError::new("app.settings_io_failed", message));
    }
    Ok(())
}

fn append_extension(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}
