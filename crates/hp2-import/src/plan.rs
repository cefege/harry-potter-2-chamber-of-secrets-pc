//! Replaces `HP2ImportPlan.cpp`'s `Generated`/`AddRequired`/
//! `ResolveLinkMode` -- the self-contained helpers `Plan`'s full
//! orchestration (not yet ported; needs the real retail/prototype data
//! trees to verify the profile-specific asset-count contracts
//! `ValidatePlan` checks) builds on.

use std::collections::BTreeMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::Path;

use crate::collect::Asset;
use crate::hash::hash_bytes;
use crate::lower::lower;
use crate::util::{fail, utf8};

/// Replaces `Generated`. Builds a fully-formed, already-hashed
/// `Asset` for in-memory content (`Default.ini`/`DefUser.ini`'s own
/// generated text, not a file on disk) -- `source`/`generated`/`link`
/// are fixed by construction, matching the C++ function always setting
/// them the same way.
pub fn generated(
    path: String,
    content: String,
    sources: Vec<String>,
    classification: String,
) -> Asset {
    let hash = hash_bytes(content.as_bytes());
    let size = content.len() as u64;
    Asset {
        source: "generated".to_string(),
        path,
        classification,
        hash,
        content,
        generated: true,
        size,
        sources,
        link: "generated".to_string(),
        ..Asset::default()
    }
}

/// Replaces `AddRequired`. `exact` is keyed by the asset's *exact*
/// (not casefolded) output path -- a different map shape than
/// [`crate::collect::insert`]'s casefold-keyed one, matching
/// `ValidatePlan`'s own local `exact` map in the C++ source.
pub fn add_required(
    exact: &BTreeMap<String, Asset>,
    path: &str,
    source: &str,
) -> Result<(), String> {
    match exact.get(path) {
        None => {
            let wanted = lower(path);
            for key in exact.keys() {
                if lower(key) == wanted {
                    return Err(fail(format!(
                        "payload has incorrect canonical case: {key}, expected {path}"
                    )));
                }
            }
            Err(fail(format!("missing required payload: {path}")))
        }
        Some(asset) => {
            if asset.source != source {
                return Err(fail(format!(
                    "{path} must be {source}-owned, found {}",
                    asset.source
                )));
            }
            Ok(())
        }
    }
}

/// Replaces `std::random_device`-seeded probe-name generation: 32 hex
/// digits, unique enough to avoid colliding with a real output file,
/// with no security/determinism requirement (this is a throwaway probe
/// filename, immediately removed). `std::collections::hash_map::
/// RandomState` already draws fresh OS-sourced random keys per
/// construction (its whole purpose is HashDoS resistance), so reading
/// an unwritten hasher's `finish()` through two instances gives 128
/// bits of OS randomness without adding a `rand` crate dependency for
/// a single non-cryptographic call site.
fn random_probe_name() -> String {
    let a = RandomState::new().build_hasher().finish();
    let b = RandomState::new().build_hasher().finish();
    format!(".hp2-link-probe-{a:016x}{b:016x}")
}

/// Replaces `ResolveLinkMode`. Probes whether `asset.disk` can be
/// hard-linked into `output_parent` by actually creating (and removing)
/// a throwaway hard link there, rather than comparing device IDs --
/// matching the C++ function's own approach exactly, including its
/// specific error handling: a cross-device probe failure falls back to
/// `"copy"` (or fails outright if the caller explicitly required
/// `Hardlink`), but *any other* probe failure still returns `"hardlink"`
/// optimistically -- the C++ source does this too (not a bug being
/// preserved, the actual documented contract: the probe failing for some
/// unrelated reason, e.g. a permissions quirk on the probe name
/// specifically, doesn't mean the real asset path will fail the same
/// way later).
///
/// The same-device success path (the common case) is verified directly:
/// this machine's `/tmp` and the repo are on the same filesystem, so a
/// real `cargo test` run exercises `fs::hard_link` actually succeeding,
/// not just the code path existing. The cross-device-link branch
/// (`ErrorKind::CrossesDevices`, confirmed stable and matching
/// `std::errc::cross_device_link`'s semantics) is implemented per its
/// documented contract but not empirically exercised -- this single-
/// filesystem dev machine has no second device to force that error on.
pub fn resolve_link_mode(
    asset: &Asset,
    output_parent: &Path,
    requested: LinkMode,
) -> Result<String, String> {
    if asset.generated {
        return Ok("generated".to_string());
    }
    if requested == LinkMode::Copy {
        return Ok("copy".to_string());
    }
    let probe = output_parent.join(random_probe_name());
    match std::fs::hard_link(&asset.disk, &probe) {
        Ok(()) => {
            std::fs::remove_file(&probe).map_err(|e| {
                fail(format!(
                    "cannot remove link-mode probe {}: {}",
                    utf8(&probe).unwrap_or_else(|_| probe.display().to_string()),
                    e
                ))
            })?;
            Ok("hardlink".to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            if requested == LinkMode::Hardlink {
                return Err(fail(format!(
                    "cannot hardlink across filesystems: {} -> {}",
                    utf8(&asset.disk).unwrap_or_else(|_| asset.disk.display().to_string()),
                    utf8(output_parent).unwrap_or_else(|_| output_parent.display().to_string())
                )));
            }
            Ok("copy".to_string())
        }
        Err(_) => Ok("hardlink".to_string()),
    }
}

/// Mirrors `HarryPotter2/Unreal/Import/Inc/HP2Import.h`'s
/// `enum class LinkMode { Auto, Copy, Hardlink };` -- same discriminants,
/// read from C++ across FFI as a `u32`.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkMode {
    Auto = 0,
    Copy = 1,
    Hardlink = 2,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_support::{tempdir, write};

    #[test]
    fn generated_hashes_content_and_sets_fixed_fields() {
        let asset = generated(
            "System/Default.ini".to_string(),
            "[Core.System]\n".to_string(),
            vec!["prototype:System/Default.ini".to_string()],
            "generated_safe_native".to_string(),
        );
        assert_eq!(asset.source, "generated");
        assert_eq!(asset.path, "System/Default.ini");
        assert_eq!(asset.classification, "generated_safe_native");
        assert!(asset.generated);
        assert_eq!(asset.link, "generated");
        assert_eq!(asset.size, 14);
        assert_eq!(asset.hash, hash_bytes(b"[Core.System]\n"));
        assert_eq!(asset.sources, vec!["prototype:System/Default.ini"]);
    }

    #[test]
    fn add_required_passes_when_the_exact_path_exists_with_the_right_source() {
        let mut exact = BTreeMap::new();
        exact.insert(
            "System/Core.u".to_string(),
            Asset {
                source: "prototype".to_string(),
                path: "System/Core.u".to_string(),
                ..Asset::default()
            },
        );
        add_required(&exact, "System/Core.u", "prototype").unwrap();
    }

    #[test]
    fn add_required_fails_on_wrong_source() {
        let mut exact = BTreeMap::new();
        exact.insert(
            "System/Core.u".to_string(),
            Asset {
                source: "retail".to_string(),
                path: "System/Core.u".to_string(),
                ..Asset::default()
            },
        );
        let err = add_required(&exact, "System/Core.u", "prototype").unwrap_err();
        assert_eq!(err, "System/Core.u must be prototype-owned, found retail");
    }

    #[test]
    fn add_required_fails_on_missing_payload() {
        let exact = BTreeMap::new();
        let err = add_required(&exact, "System/Core.u", "prototype").unwrap_err();
        assert_eq!(err, "missing required payload: System/Core.u");
    }

    #[test]
    fn add_required_distinguishes_missing_from_wrong_case() {
        let mut exact = BTreeMap::new();
        exact.insert(
            "system/core.u".to_string(),
            Asset {
                source: "prototype".to_string(),
                path: "system/core.u".to_string(),
                ..Asset::default()
            },
        );
        let err = add_required(&exact, "System/Core.u", "prototype").unwrap_err();
        assert_eq!(
            err,
            "payload has incorrect canonical case: system/core.u, expected System/Core.u"
        );
    }

    #[test]
    fn resolve_link_mode_returns_generated_for_generated_assets_without_touching_disk() {
        let asset = generated(
            "System/Default.ini".to_string(),
            "x".to_string(),
            vec![],
            "c".to_string(),
        );
        // output_parent doesn't need to exist: generated assets never
        // reach the filesystem probe.
        let mode = resolve_link_mode(&asset, Path::new("/nonexistent"), LinkMode::Auto).unwrap();
        assert_eq!(mode, "generated");
    }

    #[test]
    fn resolve_link_mode_returns_copy_when_explicitly_requested() {
        let dir = tempdir();
        write(dir.path(), "src.txt", b"x");
        let asset = Asset {
            disk: dir.path().join("src.txt"),
            ..Asset::default()
        };
        let mode = resolve_link_mode(&asset, dir.path(), LinkMode::Copy).unwrap();
        assert_eq!(mode, "copy");
    }

    #[test]
    fn resolve_link_mode_hardlinks_on_the_same_filesystem() {
        let dir = tempdir();
        write(dir.path(), "src.txt", b"x");
        let asset = Asset {
            disk: dir.path().join("src.txt"),
            ..Asset::default()
        };
        let mode = resolve_link_mode(&asset, dir.path(), LinkMode::Auto).unwrap();
        assert_eq!(mode, "hardlink");
        let mode = resolve_link_mode(&asset, dir.path(), LinkMode::Hardlink).unwrap();
        assert_eq!(mode, "hardlink");
    }

    #[test]
    fn random_probe_name_does_not_repeat_across_many_calls() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            assert!(seen.insert(random_probe_name()));
        }
    }
}
