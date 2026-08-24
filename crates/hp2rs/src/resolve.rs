//! Zero-argument launch resolution: the datadir and map token a plain
//! double-click (`open dist/macos-arm64-rs/HarryPotter2.app`) provides no
//! CLI context for.
//!
//! Datadir chain (first hit wins):
//! 1. explicit `-datadir=` (handled by the caller, never persisted),
//! 2. the launcher profile store's Retail assignment
//!    (`~/Library/Application Support/Harry Potter 2/User/Launcher.ini`,
//!    `[DataSources]`),
//! 3. a native folder picker whose validated choice is persisted back to
//!    the store so later launches skip it.
//!
//! The picker sits behind [`FolderChooser`] so headless tests can inject a
//! scripted chooser instead of a blocking `rfd` dialog.

use std::path::{Path, PathBuf};

use hp_app::store::{self, DataSource, DataSourceConfiguration};
use hp_engine::error::EngineError;

/// Prompt shown by the first-run folder picker.
const PICKER_PROMPT: &str = "Choose your Harry Potter 2 folder (contains Maps/ and System/)";

/// New Game entry map: `NEWGAME_COMMAND_LINE_TOKEN`
/// (`UnEngineWin.h:184`, mirrored by `hp_app::policy::build_selected_command`).
pub const NEW_GAME_ENTRY_MAP: &str = "PrivetDr.unr";

/// Blocking native folder chooser.
pub trait FolderChooser {
    fn choose_folder(&self, prompt: &str) -> Option<PathBuf>;
}

/// Real `rfd` dialog. Automation escape hatch: `HP2_NO_PICKER=1` declines
/// immediately so headless harnesses observe the picker decision without a
/// window server.
pub struct RfdChooser;

impl FolderChooser for RfdChooser {
    fn choose_folder(&self, prompt: &str) -> Option<PathBuf> {
        if std::env::var_os("HP2_NO_PICKER").is_some() {
            return None;
        }
        rfd::FileDialog::new().set_title(prompt).pick_folder()
    }
}

/// `<home>/Library/Application Support/Harry Potter 2/User` — the launcher
/// profile-store root (`RETAIL_IMPORT.md` layout contract).
pub fn launcher_root(home: &Path) -> PathBuf {
    home.join("Library/Application Support/Harry Potter 2/User")
}

/// A folder is a Harry Potter 2 data root when its config layer exists.
fn valid_datadir(root: &Path) -> bool {
    root.join("System").join("Default.ini").is_file()
}

/// Resolve the data root when `-datadir=` was not given. Loud at every
/// decision; persists a validated picker choice to the profile store.
pub fn resolve_datadir(
    launcher_root: &Path,
    chooser: &dyn FolderChooser,
) -> Result<PathBuf, EngineError> {
    // The store commit needs the directory; a fresh install has neither.
    std::fs::create_dir_all(launcher_root).map_err(|error| {
        EngineError::new(
            "app.store_unwritable",
            format!("{}: {error}", launcher_root.display()),
        )
    })?;
    let config = store::load_data_source_configuration(launcher_root)
        .map_err(|error| EngineError::new("app.store_unreadable", error.message().to_string()))?;

    // 1. Persisted Retail assignment.
    if !config.retail_root.is_empty() {
        let stored = PathBuf::from(&config.retail_root);
        if valid_datadir(&stored) {
            eprintln!("hp2rs: [app.datadir_from_store] using {}", stored.display());
            return Ok(stored);
        }
        eprintln!(
            "hp2rs: [app.datadir_invalid] stored retail root {} lacks System/Default.ini",
            stored.display()
        );
    }

    // 2. First-run picker fallback.
    eprintln!("hp2rs: [app.picker_shown] {PICKER_PROMPT}");
    let Some(chosen) = chooser.choose_folder(PICKER_PROMPT) else {
        return Err(EngineError::new(
            "app.datadir_missing",
            "no data folder selected; launch again to pick your Harry Potter 2 folder",
        ));
    };
    if !valid_datadir(&chosen) {
        return Err(EngineError::new(
            "app.datadir_invalid",
            format!(
                "{} is not a Harry Potter 2 folder: System/Default.ini missing",
                chosen.display()
            ),
        ));
    }

    // 3. Persist so the next launch skips the picker. The Prototype slot
    // stays untouched.
    let configuration = DataSourceConfiguration {
        selected: Some(DataSource::Retail),
        retail_root: chosen.to_string_lossy().to_string(),
        prototype_root: config.prototype_root,
    };
    store::commit_data_source_configuration(launcher_root, &configuration)
        .map_err(|error| EngineError::new("app.store_unwritable", error.message().to_string()))?;
    eprintln!("hp2rs: [app.datadir_persisted] {}", chosen.display());
    Ok(chosen)
}

/// Every `*.unr` map reachable through the engine's `[Paths]` resolution,
/// deduplicated and alphabetically sorted (case-insensitive).
pub fn enumerate_maps(data_root: &Path, ini_set: &hp_ini::IniSet) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for dir in ini_set.paths(data_root) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.to_lowercase().ends_with(".unr") && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names.sort_by_key(|name| name.to_lowercase());
    names
}

/// No map token given: prefer the C++ New Game entry map, else the first
/// alphabetical map. Returns the engine token (`Maps\Name.unr`).
pub fn autoselect_map_token(data_root: &Path) -> Result<String, EngineError> {
    let ini_set = crate::load_ini_set(data_root)?;
    let candidates = enumerate_maps(data_root, &ini_set);
    let Some(file_name) = candidates
        .iter()
        .find(|name| name.eq_ignore_ascii_case(NEW_GAME_ENTRY_MAP))
        .cloned()
        .or_else(|| candidates.first().cloned())
    else {
        return Err(EngineError::new(
            "app.map_missing",
            format!(
                "no *.unr maps found under {} via [Paths]",
                data_root.join("Maps").display()
            ),
        ));
    };
    eprintln!("hp2rs: [app.map_autoselected] Maps\\{file_name}");
    Ok(format!("Maps\\{file_name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique scratch dir under the system temp dir (no tempfile dep).
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "hp2rs-resolve-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).expect("scratch dir");
            Self(path)
        }

        fn make_datadir(label: &str) -> Self {
            let scratch = Self::new(label);
            std::fs::create_dir_all(scratch.0.join("System")).expect("System");
            std::fs::create_dir_all(scratch.0.join("Maps")).expect("Maps");
            std::fs::write(scratch.0.join("System").join("Default.ini"), []).expect("Default.ini");
            scratch
        }
    }

    struct SpyChooser {
        reply: Option<PathBuf>,
        invoked: std::cell::Cell<bool>,
    }

    impl SpyChooser {
        fn declines() -> Self {
            Self {
                reply: None,
                invoked: std::cell::Cell::new(false),
            }
        }
        fn picks(folder: &Path) -> Self {
            Self {
                reply: Some(folder.to_path_buf()),
                invoked: std::cell::Cell::new(false),
            }
        }
    }

    impl FolderChooser for SpyChooser {
        fn choose_folder(&self, _prompt: &str) -> Option<PathBuf> {
            self.invoked.set(true);
            self.reply.clone()
        }
    }

    fn seed_retail_assignment(launcher_root: &Path, retail_root: &str) {
        store::commit_data_source_configuration(
            launcher_root,
            &DataSourceConfiguration {
                selected: Some(DataSource::Retail),
                retail_root: retail_root.to_string(),
                prototype_root: String::new(),
            },
        )
        .expect("seed assignment");
    }

    #[test]
    fn stored_retail_assignment_wins_without_touching_the_picker() {
        let scratch = Scratch::make_datadir("stored");
        let launcher = Scratch::new("stored-launcher");
        seed_retail_assignment(&launcher.0, &scratch.0.to_string_lossy());

        let chooser = SpyChooser::picks(Path::new("/should/not/be/asked"));
        let resolved = resolve_datadir(&launcher.0, &chooser).expect("resolves");

        assert_eq!(resolved, scratch.0);
        assert!(!chooser.invoked.get(), "picker must stay closed");
    }

    #[test]
    fn empty_store_shows_picker_persists_validated_choice() {
        let scratch = Scratch::make_datadir("picked");
        let launcher = Scratch::new("picked-launcher");
        std::fs::create_dir_all(&launcher.0).expect("launcher root");

        let chooser = SpyChooser::picks(&scratch.0);
        let resolved = resolve_datadir(&launcher.0, &chooser).expect("resolves");

        assert_eq!(resolved, scratch.0);
        assert!(
            chooser.invoked.get(),
            "resolution must reach picker-shown state"
        );
        let reloaded = store::load_data_source_configuration(&launcher.0).expect("reload");
        assert_eq!(reloaded.retail_root, scratch.0.to_string_lossy());
        assert_eq!(reloaded.selected, Some(DataSource::Retail));
    }

    #[test]
    fn cancelled_picker_is_a_loud_error_and_persists_nothing() {
        let launcher = Scratch::new("cancel-launcher");
        std::fs::create_dir_all(&launcher.0).expect("launcher root");

        let chooser = SpyChooser::declines();
        let error = resolve_datadir(&launcher.0, &chooser).expect_err("loud cancel");

        assert_eq!(error.reason_code, "app.datadir_missing");
        assert!(chooser.invoked.get(), "the chain reaches the picker");
        let reloaded = store::load_data_source_configuration(&launcher.0).expect("reload");
        assert_eq!(reloaded.retail_root, "", "a cancel never writes the store");
    }

    #[test]
    fn invalid_pick_is_rejected_and_not_persisted() {
        let empty = Scratch::new("invalid-pick");
        let launcher = Scratch::new("invalid-launcher");
        std::fs::create_dir_all(&launcher.0).expect("launcher root");

        let chooser = SpyChooser::picks(&empty.0);
        let error = resolve_datadir(&launcher.0, &chooser).expect_err("rejected");

        assert_eq!(error.reason_code, "app.datadir_invalid");
        let reloaded = store::load_data_source_configuration(&launcher.0).expect("reload");
        assert_eq!(reloaded.retail_root, "");
    }

    #[test]
    fn invalid_stored_root_falls_through_to_picker_and_repairs_the_store() {
        let good = Scratch::make_datadir("repair");
        let launcher = Scratch::new("repair-launcher");
        seed_retail_assignment(&launcher.0, "/definitely/absent");

        let chooser = SpyChooser::picks(&good.0);
        let resolved = resolve_datadir(&launcher.0, &chooser).expect("resolves");

        assert_eq!(resolved, good.0);
        let reloaded = store::load_data_source_configuration(&launcher.0).expect("reload");
        assert_eq!(reloaded.retail_root, good.0.to_string_lossy());
    }

    #[test]
    fn map_enumeration_walks_paths_resolution_sorted() {
        let scratch = Scratch::make_datadir("maps");
        for name in ["Startup.unr", "Entry.unr", "notes.txt"] {
            std::fs::write(scratch.0.join("Maps").join(name), []).expect("map stub");
        }
        // A stray .unr in another Paths dir still counts, like the engine's
        // package scan.
        std::fs::create_dir_all(scratch.0.join("Textures")).expect("Textures");
        std::fs::write(scratch.0.join("Textures").join("Weird.unr"), []).expect("stub");
        std::fs::write(scratch.0.join("Maps").join("duplicate.txt"), []).expect("noise");

        let ini_set = crate::load_ini_set(&scratch.0).expect("ini");
        let maps = enumerate_maps(&scratch.0, &ini_set);

        assert_eq!(
            maps,
            vec![
                "Entry.unr".to_string(),
                "Startup.unr".to_string(),
                "Weird.unr".to_string(),
            ]
        );
    }

    #[test]
    fn autoselect_prefers_new_game_entry_map_then_first_alphabetical() {
        let with_entry = Scratch::make_datadir("entry");
        for name in ["Adv1Willow.unr", "PrivetDr.unr", "startup.unr"] {
            std::fs::write(with_entry.0.join("Maps").join(name), []).expect("stub");
        }
        assert_eq!(
            autoselect_map_token(&with_entry.0).expect("token"),
            "Maps\\PrivetDr.unr"
        );

        let without_entry = Scratch::make_datadir("alpha");
        for name in ["startup.unr", "Entry.unr", "Adv11aCorridor.unr"] {
            std::fs::write(without_entry.0.join("Maps").join(name), []).expect("stub");
        }
        assert_eq!(
            autoselect_map_token(&without_entry.0).expect("token"),
            "Maps\\Adv11aCorridor.unr"
        );

        let barren = Scratch::make_datadir("barren");
        assert_eq!(
            autoselect_map_token(&barren.0).unwrap_err().reason_code,
            "app.map_missing"
        );
    }
}
