//! Native port of `Build/repair_save.py`.
//!
//! The Python driver launches an external engine binary to re-save through
//! the retail loader (CLI contract: `--engine`, `--data-root`,
//! `--user-root`, positional target, `--timeout-seconds`). Here the loader
//! *is* hp-engine, so a repair reduces to its essence: validate the save
//! slot contract, reload through the structural reader, re-serialize with
//! the byte-exact writer, then back up and atomically replace the original.
//!
//! Filesystem choreography mirrors the Python driver exactly: a
//! non-clobbering timestamped backup beside the target
//! (`SaveN.usa.backup-<UTC stamp>[-k]`), then an atomic rename from a
//! hidden temporary file in the same directory.

use std::ffi::OsStr;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{SaveError, read_save, write_save};

/// Outcome of one native repair pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairReport {
    /// The save that was repaired in place.
    pub target: PathBuf,
    /// Non-clobbering backup of the original bytes created before the swap.
    pub backup: PathBuf,
    /// Numeric slot parsed from the `Save<N>.usa` name.
    pub slot: u32,
    /// True when re-serialization changed the bytes — structural corruption
    /// the writer normalized away. Payload-bit flips are *not* detectable
    /// structurally, so a clean-looking file can still carry flipped bytes;
    /// this flag reports only what the round trip itself changed.
    pub rewritten: bool,
}

/// Repair failure: either a save-slot/contract precondition or a save
/// decoding fault.
#[derive(Debug)]
pub enum RepairError {
    /// Precondition violation mirroring `Build/repair_save.py::RepairError`.
    Contract(String),
    /// The save could not be decoded or re-encoded.
    Save(SaveError),
}

impl std::fmt::Display for RepairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Contract(message) => write!(f, "repair refused: {message}"),
            Self::Save(error) => write!(f, "repair failed on save decode/encode: {error}"),
        }
    }
}

impl std::error::Error for RepairError {}

impl From<SaveError> for RepairError {
    fn from(error: SaveError) -> Self {
        Self::Save(error)
    }
}

fn contract(message: impl Into<String>) -> RepairError {
    RepairError::Contract(message.into())
}

/// `Save(0|[1-9][0-9]*)\.usa` — the canonical slot-naming contract enforced
/// by `Build/repair_save.py`. Returns the numeric slot.
pub fn parse_save_slot(file_name: &OsStr) -> Option<u32> {
    let name = file_name.to_str()?;
    let stem = name.strip_prefix("Save")?.strip_suffix(".usa")?;
    // Reject empty stems and non-canonical zero padding (Save01.usa).
    if stem.is_empty() || (stem.len() > 1 && stem.starts_with('0')) {
        return None;
    }
    stem.parse().ok()
}

/// Repair one save in place, replicating what `Build/repair_save.py`
/// drives natively: load through the reader, re-save through the
/// byte-exact writer, keep a backup, swap atomically.
pub fn repair_save(target: &Path) -> Result<RepairReport, RepairError> {
    let file_name = target
        .file_name()
        .ok_or_else(|| contract(format!("{}: not a file path", target.display())))?;
    let slot = parse_save_slot(file_name).ok_or_else(|| {
        contract(format!(
            "{}: not a canonical save slot (expected Save<N>.usa, N = 0 or decimal without \
             leading zeros)",
            target.display()
        ))
    })?;

    let metadata = fs::symlink_metadata(target)
        .map_err(|error| contract(format!("cannot inspect {}: {error}", target.display())))?;
    if metadata.is_symlink() {
        return Err(contract(format!(
            "{}: refusing to repair a symlink",
            target.display()
        )));
    }
    if !metadata.is_file() {
        return Err(contract(format!(
            "{}: not a regular file",
            target.display()
        )));
    }

    let original = fs::read(target)
        .map_err(|error| contract(format!("cannot read {}: {error}", target.display())))?;

    let archive = read_save(&original)?;
    let mut repaired = Vec::with_capacity(original.len());
    write_save(&archive, &mut repaired)?;
    let rewritten = repaired != original;

    let backup = create_backup(target, &original)?;
    atomic_replace(&repaired, target, &backup)?;

    Ok(RepairReport {
        target: target.to_path_buf(),
        backup,
        slot,
        rewritten,
    })
}

/// Copy `original` into a fresh non-clobbering
/// `{name}.backup-{stamp}[-k]` file beside `target`.
fn create_backup(target: &Path, original: &[u8]) -> Result<PathBuf, RepairError> {
    let stamp = utc_stamp();
    let mode = fs::metadata(target)
        .map_err(|error| contract(format!("cannot stat {}: {error}", target.display())))?
        .permissions();
    let name = target.file_name().map_or_else(
        || target.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );

    for suffix in 0..1000u32 {
        let discriminator = if suffix == 0 {
            String::new()
        } else {
            format!("-{suffix}")
        };
        let candidate = target.with_file_name(format!("{name}.backup-{stamp}{discriminator}"));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(contract(format!(
                    "cannot create backup {}: {error}",
                    candidate.display()
                )));
            }
        };
        let written = file
            .write_all(original)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::set_permissions(&candidate, mode));
        match written {
            Ok(()) => return Ok(candidate),
            Err(error) => {
                drop(file);
                let _ = fs::remove_file(&candidate);
                return Err(contract(format!(
                    "cannot complete backup {}: {error}",
                    candidate.display()
                )));
            }
        }
    }
    Err(contract(format!(
        "cannot choose a non-clobbering backup name beside {}",
        target.display()
    )))
}

/// Write `repaired` to a hidden temporary file beside `target`, then rename
/// it over the target atomically.
fn atomic_replace(repaired: &[u8], target: &Path, backup: &Path) -> Result<(), RepairError> {
    let directory = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target.file_name().map_or_else(
        || target.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mode = fs::metadata(target)
        .map_err(|error| contract(format!("cannot stat {}: {error}", target.display())))?
        .permissions();
    let unique = unique_suffix();

    for counter in 0..1000u32 {
        let temporary = directory.join(format!(".{name}.repair-{unique}-{counter}"));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(contract(format!(
                    "cannot create staging file {}: {error}; original remains in place and \
                     backup is {}",
                    temporary.display(),
                    backup.display()
                )));
            }
        };
        let written = file
            .write_all(repaired)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::set_permissions(&temporary, mode));
        if let Err(error) = written {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(contract(format!(
                "cannot stage replacement for {}; original remains in place and backup is {}: \
                 {error}",
                target.display(),
                backup.display()
            )));
        }
        return match fs::rename(&temporary, target) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temporary);
                Err(contract(format!(
                    "cannot atomically replace {}; original remains in place and backup is {}: \
                     {error}",
                    target.display(),
                    backup.display()
                )))
            }
        };
    }
    Err(contract(format!(
        "cannot choose a staging name beside {}",
        target.display()
    )))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// UTC timestamp in the Python driver's `%Y%m%dT%H%M%S.%fZ` shape.
fn utc_stamp() -> String {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = since_epoch.as_secs() as i64;
    let micros = since_epoch.subsec_micros();
    let days = seconds.div_euclid(86_400);
    let second_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}.{micros:06}Z",
        second_of_day / 3600,
        (second_of_day % 3600) / 60,
        second_of_day % 60
    )
}

/// Days-since-epoch to civil date (Howard Hinnant's algorithm).
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}
