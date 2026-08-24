//! Launcher profile store: `Launcher.ini` catalog persistence, launch
//! selection storage, edition-isolated profiles, and the pure legacy
//! settings migration — ported from
//! `HarryPotter2/Unreal/SDLLaunch/Src/HP2LauncherStore.cpp`.
//!
//! Layout contract (`RETAIL_IMPORT.md`): writable state lives under
//! `<launcher_root>/Profiles/{Retail,Prototype}` (plus `Explicit` for
//! transient `-datadir=` overrides); `Launcher.ini` stores folder
//! assignments (`[DataSources]`) and the persisted launch selection
//! (`[LastLaunch]`).

use std::path::{Path, PathBuf};

use crate::ini_text::LauncherDoc;

use crate::atomic_file::{is_directory, publish, read_regular};
use crate::error::AppError;
use crate::policy::{
    LaunchSelection, SelectionField, deserialize_launch_selection, serialize_launch_selection,
};

pub type Result<T, E = AppError> = std::result::Result<T, E>;

const LAUNCHER_INI: &str = "Launcher.ini";
const PROFILES_DIR: &str = "Profiles";
const EXPLICIT_PROFILE: &str = "Explicit";

/// Data source edition (`DataSource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    Retail,
    Prototype,
}

impl DataSource {
    fn name(self) -> &'static str {
        match self {
            Self::Retail => "Retail",
            Self::Prototype => "Prototype",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        if text.eq_ignore_ascii_case("Retail") {
            Some(Self::Retail)
        } else if text.eq_ignore_ascii_case("Prototype") {
            Some(Self::Prototype)
        } else {
            None
        }
    }
}

/// Folder assignments persisted under `[DataSources]`
/// (`DataSourceConfiguration`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataSourceConfiguration {
    pub selected: Option<DataSource>,
    pub retail_root: String,
    pub prototype_root: String,
}

// ------------------------------------------------------------ catalog io

struct Catalog {
    doc: LauncherDoc,
    existed: bool,
    original: Option<Vec<u8>>,
    mode: u32,
}

fn catalog_error(code: &'static str, message: String) -> AppError {
    AppError::new(code, message)
}

fn load_catalog(launcher_root: &Path) -> Result<Catalog> {
    if !is_directory(launcher_root) {
        return Err(catalog_error(
            "app.catalog_malformed",
            format!(
                "Launcher root is not a directory: '{}'",
                launcher_root.display()
            ),
        ));
    }
    let path = launcher_root.join(LAUNCHER_INI);
    match read_regular(&path)? {
        Some((bytes, mode)) => Ok(Catalog {
            doc: LauncherDoc::decode(&bytes).map_err(|message| {
                catalog_error(
                    "app.catalog_malformed",
                    format!("Unable to decode '{}': {message}", path.display()),
                )
            })?,
            existed: true,
            original: Some(bytes),
            mode,
        }),
        None => Ok(Catalog {
            doc: LauncherDoc::decode(&[]).expect("empty document always decodes"),
            existed: false,
            original: None,
            mode: 0o600,
        }),
    }
}

fn publish_catalog(launcher_root: &Path, catalog: Catalog) -> Result<()> {
    let rendered = catalog.doc.render();
    publish(
        &launcher_root.join(LAUNCHER_INI),
        &rendered,
        catalog.existed,
        catalog.original.as_deref(),
        catalog.mode,
    )
    .map_err(|error| catalog_error("app.catalog_io_failed", error.message().to_string()))
}

// -------------------------------------------------- data source catalog

fn valid_saved_root(root: &str) -> bool {
    root.starts_with('/') && !root.contains(['\r', '\n']) && root.trim() == root
}

/// Loads `[DataSources]`. Unknown spellings fall back to their defaults per
/// key instead of failing the whole catalog (`TestMalformedDataSourceCatalogRecovery`).
pub fn load_data_source_configuration(launcher_root: &Path) -> Result<DataSourceConfiguration> {
    let catalog = load_catalog(launcher_root)?;
    let mut configuration = DataSourceConfiguration::default();
    if let Some(selected) = catalog.doc.get("DataSources", "Selected")
        && let Some(source) = DataSource::parse(selected)
    {
        configuration.selected = Some(source);
    }
    let mut slots = [
        ("RetailRoot", &mut configuration.retail_root),
        ("PrototypeRoot", &mut configuration.prototype_root),
    ];
    for (key, slot) in slots.iter_mut() {
        if let Some(value) = catalog.doc.get("DataSources", key)
            && valid_saved_root(value)
        {
            **slot = value.to_string();
        }
    }
    Ok(configuration)
}

/// Commits `[DataSources]`, rejecting unknown selections and relative or
/// multiline roots without altering persisted state.
pub fn commit_data_source_configuration(
    launcher_root: &Path,
    configuration: &DataSourceConfiguration,
) -> Result<()> {
    let selected = configuration.selected.ok_or_else(|| {
        catalog_error(
            "app.catalog_invalid",
            "Data source configuration contains an invalid selection.".to_string(),
        )
    })?;
    let valid_root = |root: &str| root.is_empty() || valid_saved_root(root);
    if !valid_root(&configuration.retail_root) || !valid_root(&configuration.prototype_root) {
        return Err(catalog_error(
            "app.catalog_invalid",
            "Data source roots must be absolute, single-line paths.".to_string(),
        ));
    }
    let mut doc = load_catalog(launcher_root)?;
    doc.doc
        .set_value("DataSources", "Selected", selected.name());
    doc.doc
        .set_value("DataSources", "RetailRoot", &configuration.retail_root);
    doc.doc.set_value(
        "DataSources",
        "PrototypeRoot",
        &configuration.prototype_root,
    );
    publish_catalog(launcher_root, doc)
}

// --------------------------------------------------- launch selection io

/// Persists the launch selection under `[LastLaunch]`. A commit replaces
/// the WHOLE selection generation so stale save-coordinate rows can never
/// survive a non-Continue write.
pub fn commit_launch_selection(launcher_root: &Path, selection: &LaunchSelection) -> Result<()> {
    serialize_launch_selection(selection)
        .map_err(|error| catalog_error("app.catalog_invalid", error))?;
    let catalog = load_catalog(launcher_root)?;
    let mut doc = catalog;
    for key in ["Action", "HasSave", "SaveIndex", "SaveSlot"] {
        doc.doc.erase("LastLaunch", key);
    }
    let fields = serialize_launch_selection(selection).expect("validated above");
    for field in fields {
        doc.doc.set_value("LastLaunch", field.key, &field.value);
    }
    publish_catalog(launcher_root, doc)
}

/// A reloaded launch selection plus the malformed-store diagnostic; when
/// [`CatalogOutcome::error`] is set the selection holds the safe Quit
/// fallback.
#[derive(Debug, Clone)]
pub struct CatalogOutcome {
    pub selection: LaunchSelection,
    pub error: Option<AppError>,
}

impl CatalogOutcome {
    /// True when the store round-tripped cleanly.
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Reloads the launch selection from `[LastLaunch]`; missing catalogs are
/// reported (never implicitly created) and malformed stores fall back to
/// Quit (`LoadLaunchSelection`).
pub fn load_launch_selection(launcher_root: &Path) -> Result<CatalogOutcome> {
    let outcome = (|| -> Result<CatalogOutcome> {
        let catalog = load_catalog(launcher_root)?;
        let fields: Vec<SelectionField> = ["Action", "HasSave", "SaveIndex", "SaveSlot"]
            .into_iter()
            .filter_map(|key| {
                catalog
                    .doc
                    .get("LastLaunch", key)
                    .map(|value| SelectionField {
                        key,
                        value: value.to_string(),
                    })
            })
            .collect();
        let parsed = deserialize_launch_selection(&fields);
        Ok(CatalogOutcome {
            selection: parsed.selection,
            error: parsed
                .error
                .map(|message| catalog_error("app.catalog_malformed", message)),
        })
    })();
    match outcome {
        Ok(outcome) => Ok(outcome),
        Err(error) => Ok(CatalogOutcome {
            selection: LaunchSelection::default(),
            error: Some(error),
        }),
    }
}

// ------------------------------------------------------ profile isolation

/// Prepares the edition-specific profile root for a data source, migrating
/// legacy mutable state exactly once and recording the migration marker in
/// the catalog (`PrepareDataSourceProfile`).
pub fn prepare_data_source_profile(launcher_root: &Path, source: DataSource) -> Result<PathBuf> {
    prepare_profile(launcher_root, source.name(), true)
}

/// Prepares the isolated profile for transient `-datadir=` overrides
/// (`Profiles/Explicit`): never migrates legacy state, never touches the
/// migration marker.
pub fn prepare_explicit_profile(launcher_root: &Path) -> Result<PathBuf> {
    prepare_profile(launcher_root, EXPLICIT_PROFILE, false)
}

fn prepare_profile(launcher_root: &Path, name: &str, migrate_legacy: bool) -> Result<PathBuf> {
    let failed = |message: String| AppError::new("app.profile_prepare_failed", message);
    if !is_directory(launcher_root) {
        return Err(failed(format!(
            "Launcher root is not a directory: '{}'",
            launcher_root.display()
        )));
    }

    let profiles_root = launcher_root.join(PROFILES_DIR);
    let requested = profiles_root.join(name);
    if let Ok(metadata) = std::fs::symlink_metadata(&profiles_root)
        && !metadata.is_dir()
    {
        return Err(failed(format!(
            "Profiles root is not a directory: '{}'",
            profiles_root.display()
        )));
    }
    if let Ok(metadata) = std::fs::symlink_metadata(&requested) {
        if metadata.is_dir() {
            return Ok(requested);
        }
        return Err(failed(format!(
            "Profile root is not a directory: '{}'",
            requested.display()
        )));
    }

    // Marker decision comes first: a recorded migration means the legacy
    // tree stays where it is and the profile starts empty.
    let mut catalog = load_catalog(launcher_root)?;
    let marker = catalog.doc.get("DataSources", "LegacyProfile");
    let has_legacy_profile = match marker {
        Some(text) => {
            if DataSource::parse(text).is_none() {
                return Err(failed(
                    "Launcher data source configuration has an invalid LegacyProfile marker."
                        .to_string(),
                ));
            }
            true
        }
        None => false,
    };

    let copy_legacy = migrate_legacy && !has_legacy_profile;
    if copy_legacy {
        inspect_legacy_tree(launcher_root)?;
    }

    std::fs::create_dir_all(&profiles_root).map_err(|error| {
        failed(format!(
            "Unable to create profiles root '{}': {error}",
            profiles_root.display()
        ))
    })?;

    // Stage beside the destination, then atomically rename into place so a
    // crash can never leave a half-populated published profile.
    let temporary = profiles_root.join(format!(".{name}.tmp"));
    let _ = std::fs::remove_dir_all(&temporary);
    if copy_legacy {
        std::fs::create_dir_all(&temporary)
            .map_err(|error| failed(format!("Unable to stage profile: {error}")))?;
        for item in ["Game.ini", "User.ini", "Save", "Cache"] {
            let source_path = launcher_root.join(item);
            if std::fs::symlink_metadata(&source_path).is_err() {
                continue;
            }
            copy_tree(&source_path, &temporary.join(item)).map_err(|message| {
                let _ = std::fs::remove_dir_all(&temporary);
                failed(message)
            })?;
        }
    } else {
        std::fs::create_dir(&temporary)
            .map_err(|error| failed(format!("Unable to stage profile: {error}")))?;
    }
    if let Err(error) = std::fs::rename(&temporary, &requested) {
        let _ = std::fs::remove_dir_all(&temporary);
        return Err(failed(format!("Unable to publish profile: {error}")));
    }

    if copy_legacy {
        catalog
            .doc
            .set_value("DataSources", "LegacyProfile", source_marker(name));
        if let Err(error) = publish_catalog(launcher_root, catalog) {
            // Roll the freshly published profile back so neither a profile
            // nor a marker survives a failed publication.
            let removal = std::fs::remove_dir_all(&requested)
                .err()
                .map(|e| e.to_string());
            let mut combined = error.message().to_string();
            if let Some(removal) = removal {
                combined.push_str(" Profile rollback failed: ");
                combined.push_str(&removal);
            }
            return Err(AppError::new("app.profile_prepare_failed", combined));
        }
    }
    Ok(requested)
}

fn source_marker(name: &str) -> &'static str {
    match name {
        "Prototype" => "Prototype",
        _ => "Retail",
    }
}

/// Rejects symlinks and irregular entries anywhere inside the legacy
/// mutable-state set (`InspectLegacyTreeAt`).
fn inspect_legacy_tree(launcher_root: &Path) -> Result<()> {
    let rejected = |path: &Path| -> AppError {
        AppError::new(
            "app.profile_prepare_failed",
            format!("Irregular legacy state at '{}'", path.display()),
        )
    };
    for item in ["Game.ini", "User.ini", "Save", "Cache"] {
        let path = launcher_root.join(item);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            return Err(rejected(&path));
        }
        if metadata.is_dir() {
            let mut stack = vec![path.clone()];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).map_err(|_| rejected(&dir))? {
                    let entry = entry.map_err(|_| rejected(&dir))?;
                    let child = entry.path();
                    let metadata =
                        std::fs::symlink_metadata(&child).map_err(|_| rejected(&child))?;
                    if metadata.is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
                        return Err(rejected(&child));
                    }
                    if metadata.is_dir() {
                        stack.push(child);
                    }
                }
            }
        } else if !metadata.is_file() {
            return Err(rejected(&path));
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> std::result::Result<(), String> {
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|error| format!("Unable to inspect '{}': {error}", source.display()))?;
    if metadata.is_dir() {
        std::fs::create_dir_all(destination)
            .map_err(|error| format!("Unable to create '{}': {error}", destination.display()))?;
        for entry in std::fs::read_dir(source)
            .map_err(|error| format!("Unable to read '{}': {error}", source.display()))?
        {
            let entry = entry
                .map_err(|error| format!("Unable to iterate '{}': {error}", source.display()))?;
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        Ok(())
    } else if metadata.is_file() {
        std::fs::copy(source, destination)
            .map_err(|error| format!("Unable to copy '{}': {error}", source.display()))
            .map(|_| ())
    } else {
        Err(format!(
            "Refusing to copy irregular entry '{}'",
            source.display()
        ))
    }
}
