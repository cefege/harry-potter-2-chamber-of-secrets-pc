//! Replaces the save-discovery functions from `HP2LauncherStore.cpp`:
//! `Index`, `SaveName`, `SlotName`, `Thumbnail`, `SavesIn`, and
//! `DiscoverSaves`. These touch the filesystem only through
//! `atomic::names` (already-ported directory listing) and a per-entry
//! `lstat`, unlike `atomic.rs`'s durable-write primitives or
//! `tree.rs`'s fd-relative traversal hardening -- a much lighter
//! filesystem footprint, scoped as its own increment regardless.

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;

use crate::atomic::{join, names};
use crate::model::SaveRecord;
use crate::store::lowercase;

/// Matches `constexpr std::uintmax_t MaxThumbnailSize = 16 * 1024 *
/// 1024;`.
const MAX_THUMBNAIL_SIZE: i64 = 16 * 1024 * 1024;

/// Replaces `Index`: parses a non-negative base-10 integer, rejecting
/// an empty string, any non-digit character, and overflow past
/// `i32::MAX`.
fn index(text: &str) -> Option<i32> {
    if text.is_empty() {
        return None;
    }
    let mut result: u64 = 0;
    for c in text.bytes() {
        if !c.is_ascii_digit() {
            return None;
        }
        let digit = u64::from(c - b'0');
        if result > (i32::MAX as u64 - digit) / 10 {
            return None;
        }
        result = result * 10 + digit;
    }
    Some(result as i32)
}

/// Replaces `SaveName`. Returns `(index, stem)` on a match, where
/// `stem` is the original (not lowercased) name with its `.usa`
/// extension stripped.
fn save_name(name: &str) -> Option<(i32, String)> {
    let lower = lowercase(name);
    if lower.len() <= 8 || !lower.starts_with("save") || !lower.ends_with(".usa") {
        return None;
    }
    let digits = &lower[4..lower.len() - 4];
    let parsed = index(digits)?;
    if digits != parsed.to_string() {
        return None;
    }
    Some((parsed, name[..name.len() - 4].to_string()))
}

/// Replaces `SlotName`.
fn slot_name(name: &str) -> Option<i32> {
    let lower = lowercase(name);
    if lower.len() <= 4 || !lower.starts_with("slot") {
        return None;
    }
    let digits = &lower[4..];
    let parsed = index(digits)?;
    if digits != parsed.to_string() {
        return None;
    }
    Some(parsed)
}

/// Replaces `Thumbnail`. Looks for a same-stem `.bmp` file (case-
/// insensitive) among `names`, returning its path if it exists as a
/// regular, non-symlink file no larger than `MAX_THUMBNAIL_SIZE`.
fn thumbnail(directory: &str, names: &[String], stem: &str) -> String {
    let wanted = lowercase(&format!("{stem}.bmp"));
    for name in names {
        if lowercase(name) != wanted {
            continue;
        }
        let path = join(directory, name);
        if let Ok(info) = fs::symlink_metadata(&path)
            && info.is_file()
            && info.size() as i64 <= MAX_THUMBNAIL_SIZE
        {
            return path;
        }
    }
    String::new()
}

/// Replaces `SavesIn`: appends every recognized save file found among
/// `names` (already-listed directory entries of `directory`) to
/// `saves`.
fn saves_in(
    directory: &str,
    names: &[String],
    slot: i32,
    slotted: bool,
    saves: &mut Vec<SaveRecord>,
) {
    for name in names {
        let Some((save_index, stem)) = save_name(name) else {
            continue;
        };
        let path = join(directory, name);
        let Ok(info) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !info.is_file() || info.size() == 0 {
            continue;
        }
        let display_name = if slotted {
            format!("Slot {slot} - Save {save_index}")
        } else {
            format!("Save {save_index}")
        };
        saves.push(SaveRecord {
            slot,
            save_index,
            uses_slot_directory: slotted,
            save_path: path.clone(),
            thumbnail_path: thumbnail(directory, names, &stem),
            display_name,
            size: info.size() as i64,
            modified_seconds: info.mtime(),
        });
    }
}

/// Replaces `DiscoverSaves`.
pub fn discover_saves(user_root: &str) -> Result<Vec<SaveRecord>, String> {
    let root = join(user_root, "Save");
    let root_info = match fs::symlink_metadata(&root) {
        Ok(info) => info,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(format!(
                "Unable to inspect save directory '{root}': {}",
                strerror_of(&e)
            ));
        }
    };
    if !root_info.is_dir() {
        return Ok(Vec::new());
    }
    let root_names = names(&root, false)?;
    let mut saves = Vec::new();
    saves_in(&root, &root_names, -1, false, &mut saves);
    for name in &root_names {
        let Some(slot) = slot_name(name) else {
            continue;
        };
        let path = join(&root, name);
        match fs::symlink_metadata(&path) {
            Ok(info) if info.is_dir() => {}
            _ => continue,
        }
        let slot_names = names(&path, false)?;
        saves_in(&path, &slot_names, slot, true, &mut saves);
    }
    saves.sort_by(|a, b| {
        use std::cmp::Ordering;
        if a.uses_slot_directory != b.uses_slot_directory {
            return if a.uses_slot_directory {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        if !a.uses_slot_directory && a.save_index != b.save_index {
            return a.save_index.cmp(&b.save_index);
        }
        if a.uses_slot_directory && a.slot != b.slot {
            return a.slot.cmp(&b.slot);
        }
        if a.save_index != b.save_index {
            return a.save_index.cmp(&b.save_index);
        }
        a.save_path.cmp(&b.save_path)
    });
    Ok(saves)
}

fn strerror_of(e: &io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => hp2_ffi_support::strerror(code),
        None => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::tempdir;

    // --- discover_saves: verified against the real C++
    // `DiscoverSaves` (externally linked, called directly) on a real
    // fixture tree covering root-level saves, slotted saves, a
    // thumbnail match, a case-insensitive "SAVEn.usa" match, a
    // malformed name, a non-numeric slot directory, and a truly-empty
    // save file -- all discovered counts, fields, and the final sort
    // order matched the real C++ exactly. ---

    fn write(dir: &std::path::Path, rel: &str, content: &[u8]) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discover_saves_on_a_full_fixture_matches_cpp_ground_truth() {
        let dir = tempdir();
        let root = dir.path();
        write(root, "Save/Save1.usa", b"root save 1\n");
        write(root, "Save/Save3.usa", b"root save 3 with thumbnail\n");
        write(root, "Save/Save3.bmp", b"BM....");
        write(
            root,
            "Save/Notasave.usa",
            b"malformed - not matching SaveN.usa\n",
        );
        write(root, "Save/Save5.usa", b"\n");
        write(root, "Save/Save7.usa", b""); // truly empty -> excluded
        write(root, "Save/SAVE9.USA", b"mixed case\n");
        write(root, "Save/Slot2/Save1.usa", b"slot save 1\n");
        write(root, "Save/Slot10/Save2.usa", b"slot10 save 2\n");
        write(root, "Save/Slot10/Save2.bmp", b"BM..");
        write(root, "Save/Slot_bad/Save1.usa", b"x"); // non-numeric slot -> excluded

        let saves = discover_saves(root.to_str().unwrap()).unwrap();
        let summary: Vec<(i32, i32, bool, i64)> = saves
            .iter()
            .map(|s| (s.slot, s.save_index, s.uses_slot_directory, s.size))
            .collect();
        // Root-level saves sorted by index (1, 3, 5, 9), then slotted
        // saves sorted by slot (2, 10) -- matches the real C++'s
        // comparator exactly.
        assert_eq!(
            summary,
            vec![
                (-1, 1, false, 12),
                (-1, 3, false, 27),
                (-1, 5, false, 1),
                (-1, 9, false, 11),
                (2, 1, true, 12),
                (10, 2, true, 14),
            ]
        );
        assert_eq!(
            saves[1].thumbnail_path,
            root.join("Save/Save3.bmp").to_str().unwrap()
        );
        assert_eq!(saves[0].thumbnail_path, "");
        assert_eq!(
            saves[5].thumbnail_path,
            root.join("Save/Slot10/Save2.bmp").to_str().unwrap()
        );
        assert_eq!(saves[0].display_name, "Save 1");
        assert_eq!(saves[4].display_name, "Slot 2 - Save 1");
        assert_eq!(saves[5].display_name, "Slot 10 - Save 2");
    }

    #[test]
    fn discover_saves_on_a_missing_save_directory_returns_empty() {
        let dir = tempdir();
        let saves = discover_saves(dir.path().to_str().unwrap()).unwrap();
        assert!(saves.is_empty());
    }

    #[test]
    fn discover_saves_when_save_path_is_a_file_not_a_directory_returns_empty() {
        let dir = tempdir();
        fs::write(dir.path().join("Save"), b"x").unwrap();
        let saves = discover_saves(dir.path().to_str().unwrap()).unwrap();
        assert!(saves.is_empty());
    }

    // --- index/save_name/slot_name: direct unit coverage for the
    // string-parsing edge cases the fixture test above doesn't
    // exercise in isolation. ---

    #[test]
    fn index_rejects_empty_non_digit_and_overflow() {
        assert_eq!(index(""), None);
        assert_eq!(index("12a"), None);
        assert_eq!(index("99999999999999999999"), None);
        assert_eq!(index("0"), Some(0));
        assert_eq!(index("42"), Some(42));
    }

    #[test]
    fn save_name_rejects_names_too_short_or_with_the_wrong_prefix_or_suffix() {
        // "save.usa" (8 chars, zero digits) fails the `size() <= 8`
        // length gate -- the shortest possible digit sequence makes
        // "save1.usa" (9 chars) the minimum *valid* name.
        assert_eq!(save_name("Save.usa"), None);
        assert_eq!(save_name("Save1.usa"), Some((1, "Save1".to_string())));
        assert_eq!(save_name("NotASave1.usa"), None);
        assert_eq!(save_name("Save12.txt"), None);
        assert_eq!(save_name("Save12.usa"), Some((12, "Save12".to_string())));
    }

    #[test]
    fn save_name_rejects_non_canonical_digit_sequences() {
        // "save007.usa" parses as index 7, but "007" != "7" as text,
        // so the real C++ (and this port) rejects it rather than
        // silently accepting a non-canonical leading-zero form.
        assert_eq!(save_name("Save007.usa"), None);
    }

    #[test]
    fn slot_name_rejects_non_numeric_and_non_canonical_suffixes() {
        assert_eq!(slot_name("Slot"), None);
        assert_eq!(slot_name("SlotAbc"), None);
        assert_eq!(slot_name("Slot007"), None);
        assert_eq!(slot_name("Slot12"), Some(12));
    }
}
