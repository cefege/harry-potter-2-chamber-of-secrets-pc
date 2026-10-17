//! Replaces the remaining top-level orchestration from
//! `HP2LauncherStore.cpp` (POSIX branch): `ReadFile`, `SelectDocument`,
//! `LoadDocument`, `LoadLauncherDocument`, `PublishLauncherDocument`,
//! `LoadLauncherState`, `CommitLauncherSettings`,
//! `LoadDataSourceConfiguration`, `CommitDataSourceConfiguration`,
//! `PrepareDataSourceProfile`, `CommitLaunchSelection`,
//! `LoadLaunchSelection`. This is the glue that ties together every
//! other module in this crate (`store.rs`, `atomic.rs`, `tree.rs`,
//! `profile.rs`, `saves.rs`, `policy.rs`) -- with this module, this
//! crate's entire non-UI `HP2LauncherStore.cpp`/`HP2LaunchPolicy.cpp`
//! surface is ported.

use std::ffi::CString;
use std::fs;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use hp2_ini::launcher::Document;

use crate::atomic::{
    StagedFile, backup, directory, discard, join, publish, restore_published_file, stage,
};
use crate::model::{
    DataSource, DataSourceConfiguration, LaunchSelection, LauncherPaths, LauncherSettings,
    LauncherState,
};
use crate::policy::{
    LaunchSelectionField, deserialize_launch_selection, serialize_launch_selection,
};
use crate::profile::{
    ensure_directory_no_follow, publish_profile, remove_published_profile, stage_profile,
};
use crate::saves::discover_saves;
use crate::store::{
    apply_game, apply_user, is_absolute_path, is_valid_utf8, load_settings, parse_source,
    source_text, trim, validate_launcher_settings,
};
use crate::tree::{close_checked, inspect_legacy_tree_at, open_directory_no_follow};

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

fn fstat(fd: std::os::fd::RawFd) -> io::Result<libc::stat> {
    let mut info = MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, info.as_mut_ptr()) } == 0 {
        Ok(unsafe { info.assume_init() })
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Replaces `ReadFile`.
fn read_file(path: &str) -> Result<(Vec<u8>, u32), String> {
    match fs::symlink_metadata(path) {
        Ok(info) if !info.is_file() => {
            return Err(format!("Expected a regular non-symlink file at '{path}'."));
        }
        Ok(_) => {}
        Err(e) => return Err(error_at("Unable to inspect", path, &e)),
    }
    let c_path = cstr(path)?;
    let raw = unsafe { libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_NOFOLLOW) };
    if raw < 0 {
        return Err(error_at(
            "Unable to open",
            path,
            &io::Error::last_os_error(),
        ));
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let opened = match fstat(fd.as_raw_fd()) {
        Ok(info) if is_reg(info.st_mode) && info.st_size >= 0 => info,
        _ => return Err(format!("Unable to verify regular file '{path}'.")),
    };
    let size = opened.st_size as usize;
    let mut bytes = vec![0u8; size];
    let mut done = 0usize;
    while done < bytes.len() {
        let count = unsafe {
            libc::read(
                fd.as_raw_fd(),
                bytes[done..].as_mut_ptr().cast(),
                bytes.len() - done,
            )
        };
        if count < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error_at("Unable to read", path, &err));
        }
        if count == 0 {
            return Err(error_at(
                "Unable to read",
                path,
                &io::Error::from_raw_os_error(libc::EIO),
            ));
        }
        done += count as usize;
    }
    if let Err(e) = close_checked(fd) {
        return Err(error_at("Unable to close", path, &e));
    }
    Ok((bytes, opened.st_mode as u32 & 0o7777))
}

/// Replaces `LoadDocument`.
fn load_document(path: &str) -> Result<(Document, u32), String> {
    let (bytes, mode) = read_file(path)?;
    match Document::decode(&bytes) {
        Ok(doc) => Ok((doc, mode)),
        Err(e) => Err(format!("Unable to decode '{path}': {e}")),
    }
}

/// Replaces `SelectDocument`. Returns `(document, exists, mode)`.
fn select_document(
    paths: &LauncherPaths,
    user_name: &str,
    default_name: &str,
) -> Result<(Document, bool, u32), String> {
    let user_path = join(&paths.user_root, user_name);
    match fs::symlink_metadata(&user_path) {
        Ok(_) => {
            let (bytes, mode) = read_file(&user_path)?;
            if let Ok(doc) = Document::decode(&bytes) {
                return Ok((doc, true, mode));
            }
            let (doc, _mode) = load_document(&join(&paths.system_root, default_name))?;
            Ok((doc, true, mode))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let (doc, _mode) = load_document(&join(&paths.system_root, default_name))?;
            Ok((doc, false, 0o600))
        }
        Err(e) => Err(error_at("Unable to inspect", &user_path, &e)),
    }
}

/// Replaces `LoadLauncherDocument`. Returns `(document, exists, mode,
/// original_bytes)`.
fn load_launcher_document(launcher_root: &str) -> Result<(Document, bool, u32, Vec<u8>), String> {
    if !directory(launcher_root) {
        return Err(format!(
            "Launcher root is not a directory: '{launcher_root}'."
        ));
    }
    let path = join(launcher_root, "Launcher.ini");
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok((Document::new(), false, 0o600, Vec::new()));
        }
        Err(e) => return Err(error_at("Unable to inspect", &path, &e)),
    }
    let (original, mode) = read_file(&path)?;
    match Document::decode(&original) {
        Ok(doc) => Ok((doc, true, mode, original)),
        Err(e) => Err(format!("Unable to decode '{path}': {e}")),
    }
}

/// Replaces `PublishLauncherDocument`.
fn publish_launcher_document(
    launcher_root: &str,
    document: &Document,
    existed: bool,
    mode: u32,
    original: &[u8],
) -> Result<(), String> {
    let path = join(launcher_root, "Launcher.ini");
    let bytes = document.encode()?;
    if existed {
        backup(&path, original, mode)?;
    }
    let mut staged = stage(&path, &bytes, if existed { mode } else { 0o600 })?;
    let mut rollback = if existed {
        match stage(&path, original, mode) {
            Ok(r) => r,
            Err(e) => {
                discard(&mut staged);
                return Err(e);
            }
        }
    } else {
        StagedFile::default()
    };
    if let Err(publication_error) = publish(&mut staged) {
        let message = match restore_published_file(&path, existed, &mut rollback) {
            Ok(()) => publication_error,
            Err(restoration_error) => {
                format!("{publication_error} Rollback failed: {restoration_error}")
            }
        };
        return Err(message);
    }
    discard(&mut rollback);
    Ok(())
}

/// Replaces `LoadLauncherState`.
pub fn load_launcher_state(paths: &LauncherPaths) -> Result<LauncherState, String> {
    let (game, _game_exists, _game_mode) = select_document(paths, "Game.ini", "Default.ini")?;
    let (user, _user_exists, _user_mode) = select_document(paths, "User.ini", "DefUser.ini")?;
    let settings = load_settings(&game, &user);
    let saves = discover_saves(&paths.user_root)?;
    Ok(LauncherState { settings, saves })
}

/// Replaces `CommitLauncherSettings`.
pub fn commit_launcher_settings(
    paths: &LauncherPaths,
    settings: &LauncherSettings,
) -> Result<(), String> {
    validate_launcher_settings(settings)?;
    if !directory(&paths.user_root) {
        return Err(format!(
            "Writable user root is not a directory: '{}'.",
            paths.user_root
        ));
    }
    let (mut game, game_exists, game_mode) = select_document(paths, "Game.ini", "Default.ini")?;
    let (mut user, user_exists, user_mode) = select_document(paths, "User.ini", "DefUser.ini")?;
    let game_path = join(&paths.user_root, "Game.ini");
    let user_path = join(&paths.user_root, "User.ini");

    let mut original_game = Vec::new();
    if game_exists {
        let (bytes, _mode) = read_file(&game_path)?;
        backup(&game_path, &bytes, game_mode)?;
        original_game = bytes;
    }
    let mut original_user = Vec::new();
    if user_exists {
        let (bytes, _mode) = read_file(&user_path)?;
        backup(&user_path, &bytes, user_mode)?;
        original_user = bytes;
    }

    apply_game(&mut game, settings);
    apply_user(&mut user, settings);
    let game_bytes = game.encode()?;
    let user_bytes = user.encode()?;

    let mut staged_game = stage(
        &game_path,
        &game_bytes,
        if game_exists { game_mode } else { 0o600 },
    )?;
    let mut staged_user = match stage(
        &user_path,
        &user_bytes,
        if user_exists { user_mode } else { 0o600 },
    ) {
        Ok(s) => s,
        Err(e) => {
            discard(&mut staged_game);
            return Err(e);
        }
    };
    let mut rollback_game = if game_exists {
        match stage(&game_path, &original_game, game_mode) {
            Ok(r) => r,
            Err(e) => {
                discard(&mut staged_game);
                discard(&mut staged_user);
                return Err(e);
            }
        }
    } else {
        StagedFile::default()
    };
    let mut rollback_user = if user_exists {
        match stage(&user_path, &original_user, user_mode) {
            Ok(r) => r,
            Err(e) => {
                discard(&mut staged_game);
                discard(&mut staged_user);
                discard(&mut rollback_game);
                return Err(e);
            }
        }
    } else {
        StagedFile::default()
    };

    if let Err(publication_error) = publish(&mut staged_game) {
        let restoration = restore_published_file(&game_path, game_exists, &mut rollback_game);
        discard(&mut staged_user);
        discard(&mut rollback_user);
        let message = match restoration {
            Ok(()) => publication_error,
            Err(restoration_error) => {
                format!("{publication_error} Rollback failed: {restoration_error}")
            }
        };
        return Err(message);
    }
    if let Err(publication_error) = publish(&mut staged_user) {
        let user_restored = restore_published_file(&user_path, user_exists, &mut rollback_user);
        let game_restored = restore_published_file(&game_path, game_exists, &mut rollback_game);
        let mut message = publication_error;
        if let Err(e) = user_restored {
            message = format!("{message} User.ini rollback failed: {e}");
        }
        if let Err(e) = game_restored {
            message = format!("{message} Game.ini rollback failed: {e}");
        }
        return Err(message);
    }

    discard(&mut rollback_game);
    discard(&mut rollback_user);
    Ok(())
}

fn is_valid_data_source_root(value: &str) -> bool {
    is_absolute_path(value)
        && is_valid_utf8(value.as_bytes())
        && !value.contains(['\r', '\n'])
        && trim(value) == value
}

/// Replaces `LoadDataSourceConfiguration`.
pub fn load_data_source_configuration(
    launcher_root: &str,
) -> Result<DataSourceConfiguration, String> {
    let mut configuration = DataSourceConfiguration::default();
    let (document, _exists, _mode, _original) = load_launcher_document(launcher_root)?;
    if let Some(value) = document.get("DataSources", "Selected")
        && let Some(source) = parse_source(&value)
    {
        configuration.selected = source;
    }
    if let Some(value) = document.get("DataSources", "RetailRoot")
        && is_valid_data_source_root(&value)
    {
        configuration.retail_root = value;
    }
    if let Some(value) = document.get("DataSources", "PrototypeRoot")
        && is_valid_data_source_root(&value)
    {
        configuration.prototype_root = value;
    }
    Ok(configuration)
}

/// Replaces `CommitDataSourceConfiguration`.
pub fn commit_data_source_configuration(
    launcher_root: &str,
    configuration: &DataSourceConfiguration,
) -> Result<(), String> {
    // `source_text` is total over `DataSource`'s only two variants in
    // Rust (unlike C++'s looser int-backed enum, which could hold an
    // out-of-range value) -- the "invalid selection" branch the real
    // C++ guards against here is unreachable.
    let selected = source_text(configuration.selected);
    if !(configuration.retail_root.is_empty()
        || is_valid_data_source_root(&configuration.retail_root))
        || !(configuration.prototype_root.is_empty()
            || is_valid_data_source_root(&configuration.prototype_root))
    {
        return Err(
            "Data source roots must be absolute, valid UTF-8 single-line paths.".to_string(),
        );
    }
    let (mut document, exists, mode, original) = load_launcher_document(launcher_root)?;
    document.set("DataSources", "Selected", selected);
    document.set("DataSources", "RetailRoot", &configuration.retail_root);
    document.set(
        "DataSources",
        "PrototypeRoot",
        &configuration.prototype_root,
    );
    publish_launcher_document(launcher_root, &document, exists, mode, &original)
}

const PROFILE_LEGACY_ITEMS: &[&str] = &["Game.ini", "User.ini", "Save", "Cache"];

/// Replaces the POSIX branch of `PrepareDataSourceProfile`.
pub fn prepare_data_source_profile(
    launcher_root: &str,
    source: DataSource,
) -> Result<String, String> {
    let source_name = source_text(source);
    if !directory(launcher_root) {
        return Err(format!(
            "Launcher root is not a directory: '{launcher_root}'."
        ));
    }
    let profiles_root = join(launcher_root, "Profiles");
    let requested_profile = join(&profiles_root, source_name);

    match fs::symlink_metadata(&profiles_root) {
        Ok(info) => {
            if !info.is_dir() {
                return Err(format!(
                    "Profiles root is not a directory: '{profiles_root}'."
                ));
            }
            match fs::symlink_metadata(&requested_profile) {
                Ok(profile_info) => {
                    if !profile_info.is_dir() {
                        return Err(format!(
                            "Profile root is not a directory: '{requested_profile}'."
                        ));
                    }
                    return Ok(requested_profile);
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(error_at(
                        "Unable to inspect profile",
                        &requested_profile,
                        &e,
                    ));
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(error_at(
                "Unable to inspect profiles root",
                &profiles_root,
                &e,
            ));
        }
    }

    let (mut document, document_exists, document_mode, original_document) =
        load_launcher_document(launcher_root)?;
    let mut has_legacy_profile = false;
    if let Some(legacy_profile) = document.get("DataSources", "LegacyProfile") {
        if parse_source(&legacy_profile).is_none() {
            return Err(
                "Launcher data source configuration has an invalid LegacyProfile marker."
                    .to_string(),
            );
        }
        has_legacy_profile = true;
    }

    if !has_legacy_profile {
        let (legacy_root_fd, _mode) = open_directory_no_follow(launcher_root)?;
        let mut inspected: Result<(), String> = Ok(());
        for name in PROFILE_LEGACY_ITEMS {
            if inspected.is_ok() {
                inspected = inspect_legacy_tree_at(
                    legacy_root_fd.as_raw_fd(),
                    name,
                    &join(launcher_root, name),
                    true,
                );
            }
        }
        if let Err(e) = close_checked(legacy_root_fd)
            && inspected.is_ok()
        {
            inspected = Err(error_at("Unable to close directory", launcher_root, &e));
        }
        inspected?;
    }

    ensure_directory_no_follow(&profiles_root)?;
    let mut staged = stage_profile(launcher_root, &requested_profile, !has_legacy_profile)?;
    publish_profile(&mut staged)?;

    if !has_legacy_profile {
        document.set("DataSources", "LegacyProfile", source_name);
        if let Err(publication_error) = publish_launcher_document(
            launcher_root,
            &document,
            document_exists,
            document_mode,
            &original_document,
        ) {
            let message = match remove_published_profile(&requested_profile) {
                Ok(()) => publication_error,
                Err(removal_error) => {
                    format!("{publication_error} Profile rollback failed: {removal_error}")
                }
            };
            return Err(message);
        }
    }
    Ok(requested_profile)
}

const SELECTION_KEYS: &[&str] = &["Action", "HasSave", "SaveIndex", "SaveSlot"];

/// Replaces `CommitLaunchSelection`.
pub fn commit_launch_selection(
    launcher_root: &str,
    selection: &LaunchSelection,
) -> Result<(), String> {
    let fields = serialize_launch_selection(selection)?;
    let (mut document, exists, mode, original) = load_launcher_document(launcher_root)?;
    // A commit replaces the WHOLE selection, not individual rows: clear
    // every persisted selection key first so a non-Continue selection
    // (or a Continue without a slot) can never inherit stale
    // save-coordinate rows from the previous generation.
    for key in SELECTION_KEYS {
        document.erase("LastLaunch", key);
    }
    for field in &fields {
        document.set("LastLaunch", &field.key, &field.value);
    }
    publish_launcher_document(launcher_root, &document, exists, mode, &original)
}

/// Replaces `LoadLaunchSelection`.
pub fn load_launch_selection(launcher_root: &str) -> Result<LaunchSelection, String> {
    let (document, _exists, _mode, _original) = load_launcher_document(launcher_root)?;
    let mut fields = Vec::new();
    for key in SELECTION_KEYS {
        if let Some(value) = document.get("LastLaunch", key) {
            fields.push(LaunchSelectionField {
                key: key.to_string(),
                value,
            });
        }
    }
    deserialize_launch_selection(&fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LaunchAction, SaveRecord, ScreenMode};
    use crate::test_support::tempdir;

    fn fixture_paths(dir: &std::path::Path) -> (LauncherPaths, std::path::PathBuf) {
        let user = dir.join("user");
        let system = dir.join("system");
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(&system).unwrap();
        fs::write(system.join("Default.ini"), "[Engine.Engine]\n").unwrap();
        fs::write(system.join("DefUser.ini"), "").unwrap();
        (
            LauncherPaths {
                user_root: user.to_str().unwrap().to_string(),
                system_root: system.to_str().unwrap().to_string(),
            },
            user,
        )
    }

    // --- load_launcher_state / commit_launcher_settings: verified
    // against the real C++ (externally linked, called directly) via
    // a throwaway probe: state falls back to Default.ini/DefUser.ini
    // when no user Game.ini/User.ini exist yet, a commit writes
    // byte-for-byte identical Game.ini/User.ini content, and a
    // subsequent load reads the committed values back. ---

    #[test]
    fn load_launcher_state_falls_back_to_system_defaults() {
        let dir = tempdir();
        let (paths, _user) = fixture_paths(dir.path());
        let state = load_launcher_state(&paths).unwrap();
        assert_eq!(state.settings.screen_mode, ScreenMode::Windowed);
        assert_eq!(state.settings.render_scale, 1.0);
        assert!(state.saves.is_empty());
    }

    #[test]
    fn commit_then_load_launcher_state_round_trips() {
        let dir = tempdir();
        let (paths, _user) = fixture_paths(dir.path());
        let settings = LauncherSettings {
            screen_mode: ScreenMode::Fullscreen,
            render_scale: 0.75,
            frame_rate_limit: 60,
            ..Default::default()
        };
        commit_launcher_settings(&paths, &settings).unwrap();

        let state = load_launcher_state(&paths).unwrap();
        assert_eq!(state.settings.screen_mode, ScreenMode::Fullscreen);
        assert_eq!(state.settings.render_scale, 0.75);
        assert_eq!(state.settings.frame_rate_limit, 60);
    }

    // --- load/commit_data_source_configuration: verified against the
    // real C++ the same way -- a missing Launcher.ini yields the
    // all-default configuration, a commit writes a byte-for-byte
    // identical Launcher.ini, and a subsequent load reads it back. ---

    #[test]
    fn commit_then_load_data_source_configuration_round_trips() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        fs::create_dir_all(&launcher).unwrap();

        let missing = load_data_source_configuration(launcher.to_str().unwrap()).unwrap();
        assert_eq!(missing.selected, DataSource::Retail);
        assert_eq!(missing.retail_root, "");

        let config = DataSourceConfiguration {
            selected: DataSource::Prototype,
            retail_root: "/abs/retail".to_string(),
            prototype_root: "/abs/prototype".to_string(),
        };
        commit_data_source_configuration(launcher.to_str().unwrap(), &config).unwrap();

        let loaded = load_data_source_configuration(launcher.to_str().unwrap()).unwrap();
        assert_eq!(loaded.selected, DataSource::Prototype);
        assert_eq!(loaded.retail_root, "/abs/retail");
        assert_eq!(loaded.prototype_root, "/abs/prototype");
    }

    #[test]
    fn commit_data_source_configuration_rejects_a_relative_root() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        fs::create_dir_all(&launcher).unwrap();
        let config = DataSourceConfiguration {
            selected: DataSource::Retail,
            retail_root: "relative/path".to_string(),
            prototype_root: String::new(),
        };
        let err =
            commit_data_source_configuration(launcher.to_str().unwrap(), &config).unwrap_err();
        assert!(err.contains("must be absolute"), "unexpected error: {err}");
    }

    // --- commit/load_launch_selection: verified against the real
    // C++ the same way, including the real serialize/deserialize
    // contract's own slot-not-persisted behavior (round-tripping
    // slot=3 comes back as slot=-1, matching policy.rs's already-
    // verified LaunchSelection schema exactly). ---

    #[test]
    fn commit_then_load_launch_selection_round_trips() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        fs::create_dir_all(&launcher).unwrap();

        let selection = LaunchSelection {
            action: LaunchAction::Continue,
            has_save: true,
            save: SaveRecord {
                slot: 3,
                save_index: 7,
                ..Default::default()
            },
        };
        commit_launch_selection(launcher.to_str().unwrap(), &selection).unwrap();

        let loaded = load_launch_selection(launcher.to_str().unwrap()).unwrap();
        assert_eq!(loaded.action, LaunchAction::Continue);
        assert!(loaded.has_save);
        assert_eq!(loaded.save.save_index, 7);
        // The real C++'s own serialization schema doesn't persist
        // `slot` -- this is not a gap in this port.
        assert_eq!(loaded.save.slot, -1);
    }

    // --- prepare_data_source_profile: verified against the real C++
    // the same way on a real legacy launcher root (Game.ini/User.ini/
    // Save/Save1.usa): first call migrates the legacy tree into
    // Profiles/Retail and records the LegacyProfile marker in
    // Launcher.ini (byte-for-byte identical to the real C++'s own
    // output); a second call for a different source creates an empty
    // Profiles/Prototype (no further legacy copy, since the marker is
    // now set); a third call for the already-prepared source
    // short-circuits to the existing path. ---

    #[test]
    fn prepare_data_source_profile_migrates_legacy_tree_on_first_call() {
        let dir = tempdir();
        let launcher = dir.path().join("launcher");
        fs::create_dir_all(launcher.join("Save")).unwrap();
        fs::write(launcher.join("Game.ini"), "[Game]\n").unwrap();
        fs::write(launcher.join("User.ini"), "[User]\n").unwrap();
        fs::write(launcher.join("Save/Save1.usa"), "save1").unwrap();

        let profile_root =
            prepare_data_source_profile(launcher.to_str().unwrap(), DataSource::Retail).unwrap();
        assert_eq!(
            profile_root,
            launcher.join("Profiles/Retail").to_str().unwrap()
        );
        assert_eq!(
            fs::read(launcher.join("Profiles/Retail/Game.ini")).unwrap(),
            b"[Game]\n"
        );
        assert_eq!(
            fs::read(launcher.join("Profiles/Retail/Save/Save1.usa")).unwrap(),
            b"save1"
        );

        // A second prepare for a different source must NOT re-copy
        // the (now-irrelevant) legacy tree -- confirmed below.
        let prototype_root =
            prepare_data_source_profile(launcher.to_str().unwrap(), DataSource::Prototype).unwrap();
        assert_eq!(
            prototype_root,
            launcher.join("Profiles/Prototype").to_str().unwrap()
        );
        // No legacy copy on the second source: empty profile tree.
        let entries: Vec<_> = fs::read_dir(&prototype_root).unwrap().collect();
        assert!(entries.is_empty());

        // Re-preparing the already-prepared source short-circuits to
        // the existing path without error.
        let again =
            prepare_data_source_profile(launcher.to_str().unwrap(), DataSource::Retail).unwrap();
        assert_eq!(again, profile_root);
    }
}
