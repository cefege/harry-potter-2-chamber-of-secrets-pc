//! Save-slot durability: a previous-good `.bak` and load-time recovery from it.
//!
//! The package writer already publishes a slot atomically (`Save.tmp`, fsync, rename), so a save
//! that dies mid-write leaves the old file untouched. This module covers the remaining losses: a
//! slot that is later truncated or corrupted. Before a save replaces a slot that still parses,
//! that file is copied to `<slot>.bak` (itself staged and renamed, never half-written). When a
//! slot is about to be loaded and no longer parses, the `.bak` replaces it. Neither step touches
//! the bytes the package writer produces.

use hp2_core::package_archive::OwnedPackage;
use std::{fs, io, path::{Path, PathBuf}};

/// What [`recover_slot`] did.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Recovery {
    /// The slot parses, or it and its backup are both absent: nothing to do.
    NotNeeded,
    /// The slot was unusable and has been replaced by the `.bak`; carries the slot's old failure.
    Restored(String),
    /// The slot is unusable and there is no usable `.bak`; the slot was left as it was.
    Unrecoverable(String),
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// `Save3.usa` -> `Save3.usa.bak` (not matched by the `*.usa` slot glob).
pub(crate) fn backup_path(slot: &Path) -> PathBuf {
    with_suffix(slot, ".bak")
}

/// Whether `path` is a complete package: non-empty, header and tables parse, and every export's
/// payload range lies inside the file. A truncated tail fails here; a flipped payload bit does not.
pub(crate) fn validate(path: &Path) -> Result<(), String> {
    let length = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if length == 0 {
        return Err("file is empty".to_owned());
    }
    let package = OwnedPackage::open(path).map_err(|error| error.to_string())?;
    let summary = package.summary();
    let expected = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    if i64::from(summary.version) != i64::from(expected.version) || i64::from(summary.licensee_version) != i64::from(expected.licensee) {
        return Err(format!("unsupported version/licensee {}/{}", summary.version, summary.licensee_version));
    }
    for index in 0..package.exports().len() {
        package.export_archive(index).map_err(|error| format!("export {index}: {error}"))?;
    }
    Ok(())
}

/// Copies `from` to `to` through a sibling stage file and an atomic rename.
fn copy_atomic(from: &Path, to: &Path, stage_suffix: &str) -> io::Result<()> {
    let stage = with_suffix(to, stage_suffix);
    let result = fs::copy(from, &stage)
        .and_then(|_| fs::File::open(&stage)?.sync_all())
        .and_then(|()| fs::rename(&stage, to));
    if result.is_err() {
        let _ = fs::remove_file(&stage);
    }
    result
}

/// Call before a save replaces `slot`: keeps the current file as `.bak` if it is still a good
/// save. A slot that does not parse never overwrites an existing `.bak`. Returns whether a backup
/// was written; failures only forgo the backup and never fail the save.
pub(crate) fn preserve_previous(slot: &Path) -> bool {
    validate(slot).is_ok() && copy_atomic(slot, &backup_path(slot), ".bak.tmp").is_ok()
}

/// Call before loading `slot`: replaces a missing/corrupt slot with a good `.bak`. The unusable
/// file is kept as `<slot>.corrupt` (replacing any older one) for later inspection.
pub(crate) fn recover_slot(slot: &Path) -> Recovery {
    let failure = match validate(slot) {
        Ok(()) => return Recovery::NotNeeded,
        Err(failure) => failure,
    };
    let backup = backup_path(slot);
    if !backup.exists() {
        // No backup: a never-saved slot is not an error here, the load reports it as it always did.
        return if slot.exists() { Recovery::Unrecoverable(failure) } else { Recovery::NotNeeded };
    }
    if let Err(reason) = validate(&backup) {
        return Recovery::Unrecoverable(format!("{failure}; backup unusable: {reason}"));
    }
    let stage = with_suffix(slot, ".restore.tmp");
    if let Err(error) = fs::copy(&backup, &stage).and_then(|_| fs::File::open(&stage)?.sync_all()) {
        let _ = fs::remove_file(&stage);
        return Recovery::Unrecoverable(format!("{failure}; backup copy failed: {error}"));
    }
    if slot.exists() {
        let _ = fs::rename(slot, with_suffix(slot, ".corrupt"));
    }
    match fs::rename(&stage, slot) {
        Ok(()) => Recovery::Restored(failure),
        Err(error) => {
            let _ = fs::remove_file(&stage);
            Recovery::Unrecoverable(format!("{failure}; restore failed: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../Tests/Fixtures/save-format-golden")
            .join(name);
        fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    fn scratch(case: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hp2-save-slot-recovery-{case}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn golden_saves_validate_and_damaged_ones_do_not() {
        let dir = scratch("validate");
        for good in ["Save0.usa", "Save1.usa", "Save2.usa"] {
            let path = dir.join(good);
            fs::write(&path, fixture(good)).unwrap();
            assert_eq!(validate(&path), Ok(()), "{good}");
        }
        for bad in ["truncated_tail.usa", "empty.usa", "bad_magic.usa", "bad_version.usa", "zeroed_name_count.usa"] {
            let path = dir.join(bad);
            fs::write(&path, fixture(bad)).unwrap();
            assert!(validate(&path).is_err(), "{bad} must be rejected");
        }
        let missing = dir.join("missing.usa");
        assert!(validate(&missing).is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn preserve_previous_keeps_the_last_good_save_and_ignores_a_corrupt_one() {
        let dir = scratch("preserve");
        let slot = dir.join("Save3.usa");
        assert!(!preserve_previous(&slot), "nothing to back up before the first save");
        assert!(!backup_path(&slot).exists());

        fs::write(&slot, fixture("Save0.usa")).unwrap();
        assert!(preserve_previous(&slot));
        assert_eq!(fs::read(backup_path(&slot)).unwrap(), fixture("Save0.usa"));

        // A newer good save replaces the backup; the slot itself is never modified.
        fs::write(&slot, fixture("Save1.usa")).unwrap();
        assert!(preserve_previous(&slot));
        assert_eq!(fs::read(backup_path(&slot)).unwrap(), fixture("Save1.usa"));
        assert_eq!(fs::read(&slot).unwrap(), fixture("Save1.usa"));

        // A truncated slot must not clobber the good backup.
        fs::write(&slot, fixture("truncated_tail.usa")).unwrap();
        assert!(!preserve_previous(&slot));
        assert_eq!(fs::read(backup_path(&slot)).unwrap(), fixture("Save1.usa"));
        assert!(!with_suffix(&backup_path(&slot), ".tmp").exists(), "no stage file is left behind");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn load_falls_back_to_bak_when_the_slot_is_truncated_or_empty() {
        for (case, damaged) in [("truncated", fixture("truncated_tail.usa")), ("empty", Vec::new()), ("garbage", vec![0xAB; 64])] {
            let dir = scratch(case);
            let slot = dir.join("Save9.usa");
            fs::write(&slot, fixture("Save1.usa")).unwrap();
            assert!(preserve_previous(&slot));
            // The next save dies after the slot was damaged (or the disk corrupts it later).
            fs::write(&slot, &damaged).unwrap();

            let outcome = recover_slot(&slot);
            assert!(matches!(outcome, Recovery::Restored(_)), "{case}: {outcome:?}");
            assert_eq!(fs::read(&slot).unwrap(), fixture("Save1.usa"), "{case}: slot holds the backup bytes");
            assert_eq!(validate(&slot), Ok(()));
            assert_eq!(fs::read(backup_path(&slot)).unwrap(), fixture("Save1.usa"), "{case}: backup kept");
            assert_eq!(fs::read(with_suffix(&slot, ".corrupt")).unwrap(), damaged, "{case}: damaged file kept");
            assert_eq!(recover_slot(&slot), Recovery::NotNeeded, "{case}: second load needs nothing");
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn load_falls_back_to_bak_when_the_slot_file_is_missing() {
        let dir = scratch("missing");
        let slot = dir.join("Save2.usa");
        fs::write(backup_path(&slot), fixture("Save2.usa")).unwrap();
        assert!(matches!(recover_slot(&slot), Recovery::Restored(_)));
        assert_eq!(fs::read(&slot).unwrap(), fixture("Save2.usa"));
        assert!(!with_suffix(&slot, ".corrupt").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn recovery_leaves_everything_alone_when_nothing_usable_exists() {
        let dir = scratch("none");
        let slot = dir.join("Save4.usa");
        assert_eq!(recover_slot(&slot), Recovery::NotNeeded, "never-saved slot");

        fs::write(&slot, fixture("truncated_tail.usa")).unwrap();
        assert!(matches!(recover_slot(&slot), Recovery::Unrecoverable(_)), "corrupt without backup");
        assert_eq!(fs::read(&slot).unwrap(), fixture("truncated_tail.usa"));

        fs::write(backup_path(&slot), fixture("bad_magic.usa")).unwrap();
        assert!(matches!(recover_slot(&slot), Recovery::Unrecoverable(_)), "corrupt backup");
        assert_eq!(fs::read(&slot).unwrap(), fixture("truncated_tail.usa"));

        // A healthy slot is never replaced, even by a different valid backup.
        fs::write(&slot, fixture("Save0.usa")).unwrap();
        fs::write(backup_path(&slot), fixture("Save1.usa")).unwrap();
        assert_eq!(recover_slot(&slot), Recovery::NotNeeded);
        assert_eq!(fs::read(&slot).unwrap(), fixture("Save0.usa"));
        let _ = fs::remove_dir_all(dir);
    }
}
