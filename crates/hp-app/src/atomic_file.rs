//! Shared staged-publication helpers for launcher-owned files
//! (`Stage`/`Publish`/`Backup` from `HP2LauncherStore.cpp`).
//!
//! Contract: originals are backed up once as `<name>.bak` exactly, symlink
//! backup targets are rejected without touching them, new files are created
//! mode 0600, existing modes are retained, and publication is a same-
//! directory staged write followed by an atomic rename.

use std::path::Path;

use crate::error::AppError;

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

/// Publishes `bytes` to `destination`. When `existed`, the exact
/// `original` bytes are preserved as `<destination>.bak` on first
/// publication only (later publications never overwrite that backup), and a
/// symlinked backup target aborts the whole operation untouched.
pub fn publish(
    destination: &Path,
    bytes: &[u8],
    existed: bool,
    original: Option<&[u8]>,
    mode: u32,
) -> Result<(), AppError> {
    if existed {
        let backup_path = append_extension(destination, ".bak");
        if let Some(backup_bytes) = original {
            // Reject unsafe backup targets before touching anything.
            if let Ok(metadata) = std::fs::symlink_metadata(&backup_path) {
                if metadata.is_symlink() || !metadata.is_file() {
                    return Err(AppError::new(
                        "app.settings_io_failed",
                        format!(
                            "Backup target '{}' exists and is not a regular file",
                            backup_path.display()
                        ),
                    ));
                }
            } else {
                std::fs::write(&backup_path, backup_bytes)
                    .map_err(|error| io("Unable to write backup", &backup_path, error))?;
                set_mode(&backup_path, mode);
            }
        }
    }

    let temporary = append_extension(destination, ".tmp");
    std::fs::write(&temporary, bytes).map_err(|error| io("Unable to stage", &temporary, error))?;
    set_mode(&temporary, mode);
    if let Err(error) = std::fs::rename(&temporary, destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(io("Unable to publish", destination, error));
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
