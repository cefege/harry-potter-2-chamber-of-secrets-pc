//! Replaces `HP2ImportPlan.cpp`'s `Generated`/`AddRequired`/
//! `ResolveLinkMode`/`ValidatePlan`. `Plan` itself (the orchestration
//! that calls `CollectSide`/the cutscene collectors and builds the
//! actual asset list `ValidatePlan` checks) is not ported yet: this
//! machine has no retail data tree (`overlay-manifest.json` doesn't
//! exist anywhere in the repo, and nothing under the tree looks like
//! retail data), so end-to-end `Plan()` parity isn't something this
//! environment can verify. `ValidatePlan` itself needs no such data --
//! it's a pure function of an already-built asset list, tested here
//! with synthetic `Asset` vectors matching each profile's documented
//! count contracts, not real collected data.
//!
//! `ValidatePlan` is C++-anonymous-namespace (`namespace { ... }`),
//! so unlike `CollectSide` and the cutscene collectors it cannot be
//! called from an external probe at all -- there is no real-compiled-
//! C++ cross-check for this function the way there was for those.
//! Ported by careful line-by-line transcription instead, verified
//! against the C++ source's exact conditions and `Fail` message text,
//! with a synthetic-fixture test for the early checks every profile
//! shares (collision, invalid path, retail Ogg count) and for a
//! representative sample of each profile's own branches -- not
//! exhaustively all ~15 distinct `Fail` call sites, which would be
//! diminishing-returns repetition of the same transcription-checking
//! pattern already exercised by the ones that are covered.

use std::collections::BTreeMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::Path;

use hp2_ini::Profile;

use crate::classify::is_prohibited;
use crate::collect::Asset;
use crate::hash::hash_bytes;
use crate::lower::lower;
use crate::util::{fail, strerror_of, utf8};

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

/// Replaces `ResolveLinkMode`, **with one deliberate, user-approved
/// behavior change** (not a literal transcription at this one point):
/// the real C++ source's fall-through, after any probe failure other
/// than a cross-device link, still returns `"hardlink"` optimistically
/// -- so on a filesystem that rejects hard links for some other reason
/// (e.g. this port's own cross-check below: hard-linking a directory
/// fails with `EPERM`, not `EXDEV`), the plan would record "hardlink"
/// and the real link at commit time would then fail. Flagged to the
/// user as a likely latent bug during this port; user chose to harden
/// it here rather than preserve it: *any* probe failure, not just a
/// cross-device one, now falls back to `"copy"` (or fails outright, with
/// the real OS error text, if the caller explicitly required
/// `Hardlink`) -- the same policy the cross-device case already used,
/// just applied uniformly instead of only to that one error kind.
///
/// Everything else matches the C++ function's own approach: probing by
/// actually creating (and removing) a throwaway hard link rather than
/// comparing device IDs.
///
/// The same-device success path (the common case) is verified directly:
/// this machine's `/tmp` and the repo are on the same filesystem, so a
/// real `cargo test` run exercises `fs::hard_link` actually succeeding,
/// not just the code path existing. The cross-device-link branch
/// (`ErrorKind::CrossesDevices`, confirmed stable and matching
/// `std::errc::cross_device_link`'s semantics) is implemented per its
/// documented contract but not empirically exercised -- this single-
/// filesystem dev machine has no second device to force that error on.
/// The *other*-failure branch (the hardened one) is verified directly
/// instead, using a real non-cross-device hard-link failure found on
/// this machine: `fs::hard_link` on a directory fails with `EPERM`
/// (`ErrorKind::PermissionDenied`), confirmed via a throwaway probe.
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
                    strerror_of(&e)
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
        Err(e) => {
            if requested == LinkMode::Hardlink {
                return Err(fail(format!(
                    "cannot hardlink {} -> {}: {}",
                    utf8(&asset.disk).unwrap_or_else(|_| asset.disk.display().to_string()),
                    utf8(output_parent).unwrap_or_else(|_| output_parent.display().to_string()),
                    strerror_of(&e)
                )));
            }
            Ok("copy".to_string())
        }
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

const CRITICAL_RETAIL_PATHS: [&str; 14] = [
    "System/Core.u",
    "System/Editor.u",
    "System/Engine.u",
    "System/Fire.u",
    "System/HGame.u",
    "System/HPModels.u",
    "System/HPParticle.u",
    "System/HProps.u",
    "System/HPSounds.u",
    "System/UnrealShare.u",
    "System/UWindow.u",
    "Textures/HP2_Master.utx",
    "Textures/HP2_Menu.utx",
    "Sounds/AllDialog.USA_uax",
];
const RETAIL_ONLY_REQUIRED_PATHS: [&str; 2] = ["Maps/Entry.unr", "Textures/HP_Menu.utx"];
const RETAIL_FINAL_LOCALIZATION: [&str; 12] = [
    "System/BumpDialog.int",
    "System/BumpSet.int",
    "System/Credits.int",
    "System/Duelset.int",
    "System/hpcredits.int",
    "System/hpdialog.int",
    "System/HPMenu.int",
    "System/QuidSet.int",
    "System/BumpDialog.usa",
    "System/hpcredits.usa",
    "System/HpDialog.usa",
    "System/HpMenu.usa",
];

/// Replaces `ValidatePlan`. A pure function of an already-built asset
/// list -- see the module doc for why this is tested with synthetic
/// fixtures rather than a real compiled-C++ cross-check.
pub fn validate_plan(plan: &[Asset], profile: Profile) -> Result<(), String> {
    let mut exact: BTreeMap<String, Asset> = BTreeMap::new();
    let mut folded: BTreeMap<String, Asset> = BTreeMap::new();
    for asset in plan {
        let folded_key = lower(&asset.path);
        if let Some(prior) = folded.get(&folded_key) {
            return Err(fail(format!(
                "case-insensitive planned output collision: {} and {}",
                prior.path, asset.path
            )));
        }
        folded.insert(folded_key, asset.clone());
        let root = asset.path.split('/').next().unwrap_or("");
        if !matches!(
            root,
            "System" | "Maps" | "Textures" | "Sounds" | "Music" | "Help"
        ) || is_prohibited(&asset.path)
        {
            return Err(fail(format!("invalid planned output path: {}", asset.path)));
        }
        exact.insert(asset.path.clone(), asset.clone());
    }
    let count = |source: &str, prefix: &str, suffix: &str| -> usize {
        let prefix_key = lower(prefix);
        let suffix_key = lower(suffix);
        plan.iter()
            .filter(|a| {
                a.source == source
                    && lower(&a.path).starts_with(&prefix_key)
                    && lower(&a.path).ends_with(&suffix_key)
            })
            .count()
    };
    let retail_maps = count("retail", "Maps/", ".unr");
    let prototype_maps = count("prototype", "Maps/", ".unr");
    let retail_oggs = count("retail", "Music/", ".ogg");
    if retail_oggs != 141 {
        return Err(fail(format!(
            "retail Ogg contract mismatch: expected 141, found {retail_oggs}"
        )));
    }
    match profile {
        Profile::Full => {
            let cutscenes = count("retail", "System/Cutscenes/", ".int");
            if retail_maps != 42 || prototype_maps != 58 {
                return Err(fail(format!(
                    "full map contract mismatch: retail={retail_maps}, prototype-only={prototype_maps}"
                )));
            }
            for a in plan {
                if a.source == "prototype"
                    && lower(&a.path).starts_with("maps/")
                    && lower(&a.path).ends_with(".unr")
                    && a.classification != "prototype_only"
                {
                    return Err(fail(
                        "a full-profile prototype fallback map is not classified prototype_only",
                    ));
                }
            }
            if cutscenes != 214 {
                return Err(fail(format!(
                    "full retail cutscene contract mismatch: expected 214, found {cutscenes}"
                )));
            }
            for path in CRITICAL_RETAIL_PATHS {
                add_required(&exact, path, "retail")?;
            }
            for path in RETAIL_FINAL_LOCALIZATION {
                add_required(&exact, path, "retail")?;
            }
        }
        Profile::RetailOnly => {
            let cutscenes = count("retail", "System/Cutscenes/", ".int");
            if retail_maps != 42 || prototype_maps != 0 {
                return Err(fail(format!(
                    "retail-only map contract mismatch: retail={retail_maps}, prototype={prototype_maps}"
                )));
            }
            if cutscenes != 213 && cutscenes != 214 {
                return Err(fail(format!(
                    "retail-only cutscene contract mismatch: expected 213 or 214, found {cutscenes}"
                )));
            }
            for a in plan {
                if a.source != "retail" && a.source != "generated" {
                    return Err(fail(format!(
                        "retail-only payload has forbidden source {}: {}",
                        a.source, a.path
                    )));
                }
                if a.source == "retail" && a.classification != "retail_asset" {
                    return Err(fail(format!(
                        "retail-only asset has invalid classification: {}",
                        a.path
                    )));
                }
                if a.source == "generated" {
                    if a.path != "System/Default.ini" && a.path != "System/DefUser.ini" {
                        return Err(fail(format!(
                            "unexpected generated retail-only payload: {}",
                            a.path
                        )));
                    }
                    if a.classification != "generated_retail_native" {
                        return Err(fail(format!(
                            "generated config is not marked retail-native: {}",
                            a.path
                        )));
                    }
                    for source in &a.sources {
                        if !source.starts_with("retail:") {
                            return Err(fail(format!(
                                "generated config has a non-retail template: {}",
                                a.path
                            )));
                        }
                    }
                }
            }
            for path in CRITICAL_RETAIL_PATHS {
                add_required(&exact, path, "retail")?;
            }
            for path in RETAIL_FINAL_LOCALIZATION {
                add_required(&exact, path, "retail")?;
            }
            for path in RETAIL_ONLY_REQUIRED_PATHS {
                add_required(&exact, path, "retail")?;
            }
            add_required(&exact, "System/Default.ini", "generated")?;
            add_required(&exact, "System/DefUser.ini", "generated")?;
            for root in ["System", "Maps", "Textures", "Sounds", "Music"] {
                let found = plan
                    .iter()
                    .any(|a| a.source == "retail" && a.path.starts_with(&format!("{root}/")));
                if !found {
                    return Err(fail(format!(
                        "configured retail-only Paths category has no payload: {root}"
                    )));
                }
            }
            if retail_maps == 0 || retail_oggs == 0 {
                return Err(fail(
                    "retail-only requires at least one map and one music file",
                ));
            }
        }
        Profile::Safe => {
            let cutscenes = count("prototype", "System/Cutscenes/", ".int");
            if retail_maps != 0 || prototype_maps != 100 {
                return Err(fail(format!(
                    "safe map contract mismatch: retail={retail_maps}, prototype={prototype_maps}"
                )));
            }
            for a in plan {
                if a.source == "prototype"
                    && lower(&a.path).starts_with("maps/")
                    && lower(&a.path).ends_with(".unr")
                    && a.classification != "prototype_baseline"
                {
                    return Err(fail(
                        "a safe-profile map is not classified prototype_baseline",
                    ));
                }
            }
            if cutscenes != 185 {
                return Err(fail(format!(
                    "safe prototype cutscene contract mismatch: expected 185, found {cutscenes}"
                )));
            }
            for a in plan {
                if a.source == "retail"
                    && (lower(&a.path).starts_with("system/")
                        || lower(&a.path).starts_with("maps/")
                        || lower(&a.path).starts_with("sounds/")
                        || lower(&a.path).starts_with("help/"))
                {
                    return Err(fail(
                        "safe profile contains retail runtime packages, maps, sounds, localization, or Help",
                    ));
                }
            }
            let mut prototype_textures = 0usize;
            let mut retail_textures: Vec<String> = Vec::new();
            for a in plan {
                if a.source == "prototype" && lower(&a.path).starts_with("textures/") {
                    prototype_textures += 1;
                }
                if a.source == "retail" && lower(&a.path).starts_with("textures/") {
                    retail_textures.push(a.path.clone());
                }
            }
            if prototype_textures != 52
                || retail_textures != vec!["Textures/HP2_Menu.utx".to_string()]
            {
                return Err(fail(format!(
                    "safe texture contract mismatch: prototype={prototype_textures}, retail=[{}]",
                    retail_textures.first().map(String::as_str).unwrap_or("")
                )));
            }
            for path in [
                "System/Core.u",
                "System/Editor.u",
                "System/Engine.u",
                "System/Fire.u",
                "System/HGame.u",
                "System/HPModels.u",
                "System/HPParticle.u",
                "System/HProps.u",
                "System/HPSounds.u",
                "System/UnrealShare.u",
                "System/UWindow.u",
                "Textures/HP2_Master.utx",
                "Sounds/AllDialog.uax",
            ] {
                add_required(&exact, path, "prototype")?;
            }
            add_required(&exact, "Textures/HP2_Menu.utx", "retail")?;
            for key in exact.keys() {
                if key == "Sounds/AllDialog.USA_uax" || lower(key).ends_with(".usa") {
                    return Err(fail(
                        "safe profile must use prototype INT/AllDialog localization, not retail USA",
                    ));
                }
            }
        }
    }
    Ok(())
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
    #[cfg(unix)]
    fn resolve_link_mode_falls_back_to_copy_on_a_non_cross_device_hard_link_failure() {
        // fs::hard_link on a directory fails with EPERM
        // (ErrorKind::PermissionDenied), confirmed via a throwaway probe
        // against this machine's real filesystem -- a real, reachable
        // non-cross-device failure, not a synthetic/mocked one. This is
        // the hardened path: before the user-approved fix, any failure
        // here other than ErrorKind::CrossesDevices fell through to
        // "hardlink" optimistically; now it falls back to "copy" the
        // same way the cross-device case already did.
        let dir = tempdir();
        std::fs::create_dir(dir.path().join("a_directory")).unwrap();
        let asset = Asset {
            disk: dir.path().join("a_directory"),
            ..Asset::default()
        };
        let mode = resolve_link_mode(&asset, dir.path(), LinkMode::Auto).unwrap();
        assert_eq!(mode, "copy");
    }

    #[test]
    #[cfg(unix)]
    fn resolve_link_mode_fails_outright_on_a_non_cross_device_failure_when_hardlink_is_required() {
        let dir = tempdir();
        std::fs::create_dir(dir.path().join("a_directory")).unwrap();
        let asset = Asset {
            disk: dir.path().join("a_directory"),
            ..Asset::default()
        };
        let err = resolve_link_mode(&asset, dir.path(), LinkMode::Hardlink).unwrap_err();
        assert!(err.starts_with("cannot hardlink "), "{err}");
        assert!(!err.contains("os error"), "{err}");
    }

    #[test]
    fn random_probe_name_does_not_repeat_across_many_calls() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            assert!(seen.insert(random_probe_name()));
        }
    }

    fn asset(source: &str, path: &str, classification: &str) -> Asset {
        Asset {
            source: source.to_string(),
            path: path.to_string(),
            classification: classification.to_string(),
            ..Asset::default()
        }
    }

    fn many(count: usize, source: &str, root: &str, ext: &str, classification: &str) -> Vec<Asset> {
        // Embeds `source` in the filename so two `many()` calls with the
        // same root/ext/index range (e.g. retail and prototype Maps/)
        // never collide at the same path.
        (0..count)
            .map(|i| {
                asset(
                    source,
                    &format!("{root}/{source}-file{i:04}.{ext}"),
                    classification,
                )
            })
            .collect()
    }

    #[test]
    fn validate_plan_fails_on_a_case_insensitive_collision_before_any_count_check() {
        let plan = vec![
            asset("prototype", "System/Game.u", ""),
            asset("prototype", "SYSTEM/GAME.U", ""),
        ];
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(
            err,
            "case-insensitive planned output collision: System/Game.u and SYSTEM/GAME.U"
        );
    }

    #[test]
    fn validate_plan_fails_on_a_root_outside_the_six_allowed_categories() {
        let plan = vec![asset("prototype", "BadRoot/file.txt", "")];
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(err, "invalid planned output path: BadRoot/file.txt");
    }

    #[test]
    fn validate_plan_fails_on_a_prohibited_path_under_an_otherwise_valid_root() {
        let plan = vec![asset("prototype", "System/payload.exe", "")];
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(err, "invalid planned output path: System/payload.exe");
    }

    #[test]
    fn validate_plan_fails_on_the_retail_ogg_count_before_any_profile_branch() {
        // Checked before the profile match, so this applies identically
        // regardless of which profile is passed.
        let err = validate_plan(&[], Profile::Safe).unwrap_err();
        assert_eq!(err, "retail Ogg contract mismatch: expected 141, found 0");
    }

    /// A fully valid Safe-profile plan: every count contract and every
    /// `AddRequired` path satisfied simultaneously. Built once and reused
    /// (cloned, then perturbed) by the negative tests below instead of
    /// hand-writing a fresh large fixture per test.
    fn valid_safe_plan() -> Vec<Asset> {
        let mut plan = Vec::new();
        plan.extend(many(141, "retail", "Music", "ogg", ""));
        plan.extend(many(100, "prototype", "Maps", "unr", "prototype_baseline"));
        plan.extend(many(185, "prototype", "System/Cutscenes", "int", ""));
        // 51 + the required Textures/HP2_Master.utx below = 52 total
        // prototype textures (that required path is itself a
        // "textures/"-prefixed prototype asset, counted the same way).
        plan.extend(many(51, "prototype", "Textures", "utx", ""));
        plan.push(asset("retail", "Textures/HP2_Menu.utx", ""));
        for path in [
            "System/Core.u",
            "System/Editor.u",
            "System/Engine.u",
            "System/Fire.u",
            "System/HGame.u",
            "System/HPModels.u",
            "System/HPParticle.u",
            "System/HProps.u",
            "System/HPSounds.u",
            "System/UnrealShare.u",
            "System/UWindow.u",
            "Textures/HP2_Master.utx",
            "Sounds/AllDialog.uax",
        ] {
            plan.push(asset("prototype", path, ""));
        }
        plan
    }

    #[test]
    fn validate_plan_accepts_a_fully_valid_safe_profile_plan() {
        validate_plan(&valid_safe_plan(), Profile::Safe).unwrap();
    }

    #[test]
    fn validate_plan_fails_when_a_safe_required_prototype_payload_is_missing() {
        let mut plan = valid_safe_plan();
        plan.retain(|a| a.path != "System/Core.u");
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(err, "missing required payload: System/Core.u");
    }

    #[test]
    fn validate_plan_fails_when_safe_profile_contains_a_retail_system_asset() {
        let mut plan = valid_safe_plan();
        plan.push(asset("retail", "System/Extra.u", ""));
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(
            err,
            "safe profile contains retail runtime packages, maps, sounds, localization, or Help"
        );
    }

    #[test]
    fn validate_plan_fails_when_safe_profile_has_a_usa_localization_file() {
        let mut plan = valid_safe_plan();
        // Swap the one allowed retail texture for a disallowed .usa file
        // under a required exact path so the earlier AddRequired checks
        // still pass before reaching the USA scan.
        plan.push(asset("prototype", "Sounds/Extra.usa", ""));
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(
            err,
            "safe profile must use prototype INT/AllDialog localization, not retail USA"
        );
    }

    #[test]
    fn validate_plan_fails_when_a_safe_map_is_not_classified_prototype_baseline() {
        let mut plan = valid_safe_plan();
        // Corrupt one map's classification; it's still counted toward
        // the 100-map total (classification doesn't affect `count`), so
        // this reaches the classification check, not the count check.
        for a in &mut plan {
            if a.path == "Maps/prototype-file0000.unr" {
                a.classification = "wrong".to_string();
            }
        }
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(
            err,
            "a safe-profile map is not classified prototype_baseline"
        );
    }

    #[test]
    fn validate_plan_full_profile_requires_classified_prototype_only_fallback_maps() {
        let mut plan = Vec::new();
        plan.extend(many(141, "retail", "Music", "ogg", ""));
        plan.extend(many(42, "retail", "Maps", "unr", ""));
        plan.extend(many(58, "prototype", "Maps", "unr", "wrong_classification"));
        let err = validate_plan(&plan, Profile::Full).unwrap_err();
        assert_eq!(
            err,
            "a full-profile prototype fallback map is not classified prototype_only"
        );
    }

    #[test]
    fn validate_plan_retail_only_rejects_a_forbidden_source() {
        let mut plan = Vec::new();
        plan.extend(many(141, "retail", "Music", "ogg", "retail_asset"));
        plan.extend(many(42, "retail", "Maps", "unr", "retail_asset"));
        plan.extend(many(
            213,
            "retail",
            "System/Cutscenes",
            "int",
            "retail_asset",
        ));
        plan.push(asset("prototype", "System/Leftover.u", ""));
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "retail-only payload has forbidden source prototype: System/Leftover.u"
        );
    }

    #[test]
    fn validate_plan_retail_only_map_count_mismatch_is_reported_with_both_counts() {
        let plan = many(141, "retail", "Music", "ogg", "");
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "retail-only map contract mismatch: retail=0, prototype=0"
        );
    }

    /// A fully valid Full-profile plan: every count contract and every
    /// `AddRequired` path satisfied simultaneously.
    fn valid_full_plan() -> Vec<Asset> {
        let mut plan = Vec::new();
        plan.extend(many(141, "retail", "Music", "ogg", ""));
        plan.extend(many(42, "retail", "Maps", "unr", ""));
        plan.extend(many(58, "prototype", "Maps", "unr", "prototype_only"));
        plan.extend(many(214, "retail", "System/Cutscenes", "int", ""));
        for path in CRITICAL_RETAIL_PATHS {
            plan.push(asset("retail", path, ""));
        }
        for path in RETAIL_FINAL_LOCALIZATION {
            plan.push(asset("retail", path, ""));
        }
        plan
    }

    #[test]
    fn validate_plan_accepts_a_fully_valid_full_profile_plan() {
        validate_plan(&valid_full_plan(), Profile::Full).unwrap();
    }

    #[test]
    fn validate_plan_full_map_contract_mismatch_is_reported_with_both_counts() {
        let plan = many(141, "retail", "Music", "ogg", "");
        let err = validate_plan(&plan, Profile::Full).unwrap_err();
        assert_eq!(
            err,
            "full map contract mismatch: retail=0, prototype-only=0"
        );
    }

    #[test]
    fn validate_plan_full_retail_cutscene_count_mismatch() {
        let mut plan = valid_full_plan();
        plan.retain(|a| !a.path.starts_with("System/Cutscenes/"));
        let err = validate_plan(&plan, Profile::Full).unwrap_err();
        assert_eq!(
            err,
            "full retail cutscene contract mismatch: expected 214, found 0"
        );
    }

    #[test]
    fn validate_plan_full_fails_when_a_critical_retail_path_is_missing() {
        let mut plan = valid_full_plan();
        plan.retain(|a| a.path != "System/Core.u");
        let err = validate_plan(&plan, Profile::Full).unwrap_err();
        assert_eq!(err, "missing required payload: System/Core.u");
    }

    #[test]
    fn validate_plan_full_fails_when_a_retail_final_localization_path_is_missing() {
        let mut plan = valid_full_plan();
        plan.retain(|a| a.path != "System/Credits.int");
        let err = validate_plan(&plan, Profile::Full).unwrap_err();
        assert_eq!(err, "missing required payload: System/Credits.int");
    }

    /// A fully valid RetailOnly-profile plan: every count contract,
    /// classification rule, and every `AddRequired` path satisfied
    /// simultaneously. `Maps/Entry.unr` (from `RETAIL_ONLY_REQUIRED_PATHS`)
    /// is pushed separately, not via `many()`, since it counts toward the
    /// 42-map total alongside the 41 generic ones below (41 + 1 = 42).
    fn valid_retail_only_plan() -> Vec<Asset> {
        let mut plan = Vec::new();
        plan.extend(many(141, "retail", "Music", "ogg", "retail_asset"));
        plan.extend(many(41, "retail", "Maps", "unr", "retail_asset"));
        plan.extend(many(
            214,
            "retail",
            "System/Cutscenes",
            "int",
            "retail_asset",
        ));
        for path in CRITICAL_RETAIL_PATHS {
            plan.push(asset("retail", path, "retail_asset"));
        }
        for path in RETAIL_FINAL_LOCALIZATION {
            plan.push(asset("retail", path, "retail_asset"));
        }
        for path in RETAIL_ONLY_REQUIRED_PATHS {
            plan.push(asset("retail", path, "retail_asset"));
        }
        let mut default_ini = generated(
            "System/Default.ini".to_string(),
            "[Core.System]\n".to_string(),
            vec!["retail:System/default.ini".to_string()],
            String::new(),
        );
        default_ini.classification = "generated_retail_native".to_string();
        plan.push(default_ini);
        let mut defuser_ini = generated(
            "System/DefUser.ini".to_string(),
            "[Core.System]\n".to_string(),
            vec!["retail:System/DefUser.ini".to_string()],
            String::new(),
        );
        defuser_ini.classification = "generated_retail_native".to_string();
        plan.push(defuser_ini);
        plan
    }

    #[test]
    fn validate_plan_accepts_a_fully_valid_retail_only_plan() {
        validate_plan(&valid_retail_only_plan(), Profile::RetailOnly).unwrap();
    }

    #[test]
    fn validate_plan_retail_only_cutscene_count_mismatch() {
        let mut plan = valid_retail_only_plan();
        plan.retain(|a| !a.path.starts_with("System/Cutscenes/"));
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "retail-only cutscene contract mismatch: expected 213 or 214, found 0"
        );
    }

    #[test]
    fn validate_plan_retail_only_rejects_a_wrong_classification_on_a_retail_asset() {
        let mut plan = valid_retail_only_plan();
        for a in &mut plan {
            if a.path == "System/Core.u" {
                a.classification = "wrong".to_string();
            }
        }
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "retail-only asset has invalid classification: System/Core.u"
        );
    }

    #[test]
    fn validate_plan_retail_only_rejects_an_unexpected_generated_payload() {
        let mut plan = valid_retail_only_plan();
        let mut extra = generated(
            "System/Extra.ini".to_string(),
            "x".to_string(),
            vec!["retail:System/extra.ini".to_string()],
            String::new(),
        );
        extra.classification = "generated_retail_native".to_string();
        plan.push(extra);
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "unexpected generated retail-only payload: System/Extra.ini"
        );
    }

    #[test]
    fn validate_plan_retail_only_rejects_a_generated_config_not_marked_retail_native() {
        let mut plan = valid_retail_only_plan();
        for a in &mut plan {
            if a.path == "System/Default.ini" {
                a.classification = "wrong".to_string();
            }
        }
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "generated config is not marked retail-native: System/Default.ini"
        );
    }

    #[test]
    fn validate_plan_retail_only_rejects_a_generated_config_with_a_non_retail_template() {
        let mut plan = valid_retail_only_plan();
        for a in &mut plan {
            if a.path == "System/Default.ini" {
                a.sources = vec!["prototype:System/Default.ini".to_string()];
            }
        }
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(
            err,
            "generated config has a non-retail template: System/Default.ini"
        );
    }

    #[test]
    fn validate_plan_retail_only_fails_when_a_required_path_is_missing() {
        let mut plan = valid_retail_only_plan();
        plan.retain(|a| a.path != "Textures/HP_Menu.utx");
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(err, "missing required payload: Textures/HP_Menu.utx");
    }

    #[test]
    fn validate_plan_retail_only_paths_category_check_is_unreachable_given_the_required_path_lists()
    {
        // The "configured retail-only Paths category has no payload"
        // check (one per root: System/Maps/Textures/Sounds/Music) runs
        // *after* every AddRequired call for CRITICAL_RETAIL_PATHS,
        // RETAIL_FINAL_LOCALIZATION, and RETAIL_ONLY_REQUIRED_PATHS, and
        // after the unconditional retail-Ogg-count-141 check earlier in
        // the function. Every one of those five roots already has at
        // least one required retail-source path baked into those lists
        // (System/Core.u, Maps/Entry.unr, Textures/HP2_Master.utx,
        // Sounds/AllDialog.USA_uax, and the 141 required Music oggs) --
        // so by the time this check runs, it can never actually fail:
        // removing the one retail Sounds asset the baseline has doesn't
        // reach this check at all, it fails the earlier AddRequired
        // check for that same path instead, confirmed by actually
        // running this exact fixture (the original version of this test
        // asserted the "no payload" message and failed, with the real
        // output being "missing required payload: Sounds/AllDialog.USA_uax"
        // -- this is that finding, not a guess). This mirrors the real
        // C++ source exactly: the same redundancy exists there too (read
        // directly, not inferred), so this isn't a porting bug to fix,
        // just a dead branch this port faithfully carries over.
        let mut plan = valid_retail_only_plan();
        plan.retain(|a| a.path != "Sounds/AllDialog.USA_uax");
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(err, "missing required payload: Sounds/AllDialog.USA_uax");
    }

    #[test]
    fn validate_plan_retail_only_final_count_check_is_unreachable_given_the_ogg_count_check() {
        // Similarly, "retail-only requires at least one map and one
        // music file" (checked last, after everything else) can only
        // fire when retail_maps==0 or retail_oggs==0 -- but
        // retail_oggs is already required to be exactly 141
        // unconditionally, before the profile match even runs, and
        // retail_maps==42 is enforced by this same function's own
        // earlier "retail-only map contract mismatch" check. So by the
        // time this final check would run, both counts are already
        // pinned to nonzero values; it cannot fail. Confirmed the same
        // way as the sibling test above: the original version of this
        // test asserted the "requires at least one" message and failed,
        // with the real output being the retail Ogg count check firing
        // first (retail_oggs==0 here, checked before any profile-
        // specific logic at all). Faithful to the real C++ source, which
        // has the identical redundancy.
        let mut plan = Vec::new();
        plan.extend(many(
            214,
            "retail",
            "System/Cutscenes",
            "int",
            "retail_asset",
        ));
        for path in CRITICAL_RETAIL_PATHS {
            plan.push(asset("retail", path, "retail_asset"));
        }
        for path in RETAIL_FINAL_LOCALIZATION {
            plan.push(asset("retail", path, "retail_asset"));
        }
        plan.extend(many(42, "retail", "Maps", "unr", "retail_asset"));
        let err = validate_plan(&plan, Profile::RetailOnly).unwrap_err();
        assert_eq!(err, "retail Ogg contract mismatch: expected 141, found 0");
    }

    #[test]
    fn validate_plan_safe_map_contract_mismatch_is_reported_with_both_counts() {
        let plan = many(141, "retail", "Music", "ogg", "");
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(err, "safe map contract mismatch: retail=0, prototype=0");
    }

    #[test]
    fn validate_plan_safe_cutscene_count_mismatch() {
        let mut plan = valid_safe_plan();
        plan.retain(|a| !a.path.starts_with("System/Cutscenes/"));
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        assert_eq!(
            err,
            "safe prototype cutscene contract mismatch: expected 185, found 0"
        );
    }

    #[test]
    fn validate_plan_safe_texture_contract_mismatch_lists_only_the_first_retail_texture() {
        let mut plan = valid_safe_plan();
        // Add a second retail texture; the message must still show only
        // the *first* retail texture found (C++'s `.front()`), not a
        // joined list of all of them.
        plan.push(asset("retail", "Textures/Another.utx", ""));
        let err = validate_plan(&plan, Profile::Safe).unwrap_err();
        // BTreeMap/Vec iteration order for `plan` itself is insertion
        // order (a plain `&[Asset]` slice, not a map), so
        // "Textures/HP2_Menu.utx" (pushed before "Textures/Another.utx"
        // in valid_safe_plan) is the first retail texture found.
        assert_eq!(
            err,
            "safe texture contract mismatch: prototype=52, retail=[Textures/HP2_Menu.utx]"
        );
    }
}
