//! Rust-owned configuration cache, preserving the engine INI map and file semantics.
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::ansi::Tchar;
unsafe extern "C" {
    fn towlower(character: Tchar) -> Tchar;
}
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
struct ConfigValue {
    spelling: Vec<u32>,
    value: Vec<u32>,
}

#[derive(Clone, Debug)]
struct ConfigSection {
    spelling: Vec<u32>,
    values: Vec<ConfigValue>,
}

#[derive(Clone, Debug)]
struct ConfigFile {
    key: Vec<u32>,
    path: PathBuf,
    sections: Vec<ConfigSection>,
    dirty: bool,
    no_save: bool,
    // Exact on-disk bytes observed at load or the last successful publish.
    original: Option<Vec<u8>>,
}

/// Owns ordered INI files and the System/User path aliases used by Core.
#[derive(Clone, Debug)]
pub struct CoreConfig {
    data_root: PathBuf,
    system_ini: PathBuf,
    user_ini: PathBuf,
    files: Vec<ConfigFile>,
    language: Vec<u32>,
}

impl CoreConfig {
    /// Creates an empty cache without reading or writing files.
    pub fn new(data_root: PathBuf, system_ini: PathBuf, user_ini: PathBuf) -> Self {
        Self {
            data_root,
            system_ini,
            user_ini,
            files: Vec::new(),
            language: "int".encode_utf16().map(u32::from).collect(),
        }
    }

    fn fresh(&self) -> Self {
        Self {
            data_root: self.data_root.clone(), system_ini: self.system_ini.clone(),
            user_ini: self.user_ini.clone(), files: Vec::new(), language: self.language.clone(),
        }
    }

    /// Returns the configured language name as an owned UTF-32 snapshot.
    pub fn language_text(&self) -> Vec<u32> {
        self.language.clone()
    }

    /// Updates the language name, enforcing the native 64-TCHAR input limit.
    pub fn set_language(&mut self, text: &[u32]) -> RuntimeResult<()> {
        let text = terminated(text);
        if text.len() >= 64 || text.iter().any(|unit| char::from_u32(*unit).is_none()) {
            return Err(RuntimeError::new("Invalid language name"));
        }
        if equal_fold(&self.language, text) {
            return Ok(());
        }
        self.language.clear();
        self.language.extend_from_slice(text);
        Ok(())
    }

    /// Builds the explicit package localization filename used by `Localize`.
    pub fn localization_filename_for(&self, package: &[u32], language: Option<&[u32]>) -> RuntimeResult<PathBuf> {
        let package = units_to_string(terminated(package))?;
        let language = language.map(terminated).unwrap_or(&self.language);
        let language = units_to_string(language)?;
        Ok(self.data_root.join("System").join(format!("{package}.{language}")))
    }


    /// Distinguishes a present empty section from an absent section.
    pub fn has_section(&mut self, filename: Option<&[u32]>, section: &[u32]) -> RuntimeResult<bool> {
        let (key, path) = self.resolve_filename(filename)?;
        let Some(index) = self.file_index(&key, &path, false)? else { return Ok(false) };
        Ok(self.files[index].sections.iter()
            .any(|entry| equal_fold(&entry.spelling, terminated(section))))
    }

    /// Loads the newest case-insensitive key match, without merging different files.
    pub fn get(&mut self, filename: Option<&[u32]>, section: &[u32], key: &[u32]) -> RuntimeResult<Option<Vec<u32>>> {
        let (key_name, path) = self.resolve_filename(filename)?;
        let Some(index) = self.file_index(&key_name, &path, false)? else { return Ok(None) };
        let file = &self.files[index];
        let Some(section) = file.sections.iter().find(|entry| equal_fold(&entry.spelling, terminated(section))) else {
            return Ok(None);
        };
        Ok(section.values.iter().rev().find(|entry| equal_fold(&entry.spelling, terminated(key)))
            .map(|entry| entry.value.clone()))
    }

    /// Sets the newest matching duplicate as the native hash-chain lookup selects it.
    pub fn set(&mut self, filename: Option<&[u32]>, section: &[u32], key: &[u32], value: &[u32]) -> RuntimeResult<()> {
        let (filename_key, path) = self.resolve_filename(filename)?;
        let index = self.file_index(&filename_key, &path, true)?.ok_or_else(|| RuntimeError::new("Configuration file was not created"))?;
        let section_name = terminated(section);
        let key_name = terminated(key);
        let value = terminated(value).to_vec();
        let file = &mut self.files[index];
        let section_index = match file.sections.iter().position(|entry| equal_fold(&entry.spelling, section_name)) {
            Some(index) => index,
            None => {
                file.sections.push(ConfigSection { spelling: section_name.to_vec(), values: Vec::new() });
                file.sections.len() - 1
            }
        };
        let section = &mut file.sections[section_index];
        if let Some(existing) = section.values.iter_mut().rev().find(|entry| equal_fold(&entry.spelling, key_name)) {
            if !equal_fold(&existing.value, &value) {
                file.dirty = existing.value != value;
                existing.value = value;
            }
        } else {
            section.values.push(ConfigValue { spelling: key_name.to_vec(), value });
            file.dirty = true;
        }
        Ok(())
    }

    /// Removes all case-insensitive matches from a section, retaining other entries.
    pub fn remove_key(&mut self, filename: Option<&[u32]>, section_name: &[u32], key: &[u32]) -> RuntimeResult<()> {
        let (filename_key, path) = self.resolve_filename(filename)?;
        let index = self.file_index(&filename_key, &path, true)?
            .ok_or_else(|| RuntimeError::new("Configuration file was not created"))?;
        let file = &mut self.files[index];
        let ConfigFile { sections, dirty, .. } = file;
        let section_name = terminated(section_name);
        let Some(section_index) = sections.iter().position(|entry| equal_fold(&entry.spelling, section_name)) else {
            sections.push(ConfigSection { spelling: section_name.to_vec(), values: Vec::new() });
            *dirty = true;
            return Ok(());
        };
        let section = &mut sections[section_index];
        let old_len = section.values.len();
        section.values.retain(|entry| !equal_fold(&entry.spelling, terminated(key)));
        if section.values.len() != old_len { *dirty = true; }
        Ok(())
    }

    /// Appends a distinct duplicate entry, as native array SaveConfig does.
    pub fn append_value(&mut self, filename: Option<&[u32]>, section: &[u32], key: &[u32], value: &[u32]) -> RuntimeResult<()> {
        let (filename_key, path) = self.resolve_filename(filename)?;
        let index = self.file_index(&filename_key, &path, true)?.ok_or_else(|| RuntimeError::new("Configuration file was not created"))?;
        let section_name = terminated(section);
        let section_index = match self.files[index].sections.iter().position(|entry| equal_fold(&entry.spelling, section_name)) {
            Some(index) => index,
            None => {
                self.files[index].sections.push(ConfigSection { spelling: section_name.to_vec(), values: Vec::new() });
                self.files[index].sections.len() - 1
            }
        };
        self.files[index].sections[section_index].values.push(ConfigValue {
            spelling: terminated(key).to_vec(),
            value: terminated(value).to_vec(),
        });
        self.files[index].dirty = true;
        Ok(())
    }

    /// Flushes cached files, optionally selecting a filename and reloading afterward.
    /// Each dirty file is staged and renamed separately; this is not a multi-file
    /// transaction. External byte changes are checked, but a non-cooperating
    /// writer can race the final check and rename. Callers must serialize writers.
    /// Formatting/comments are not retained by this native-style config cache.
    pub fn flush(&mut self, filename: Option<&[u32]>, reload: bool) -> RuntimeResult<()> {
        // Native flush compares its raw filename to the stored cache key; it does
        // not apply Find's extension or System/User alias normalization here.
        let selected = filename.map(|name| terminated(name).to_vec());
        for index in 0..self.files.len() {
            let file = &mut self.files[index];
            if selected.as_ref().is_none_or(|key| equal_fold(key, &file.key)) {
                if let Some(bytes) = prepare_file(file)? {
                    publish_file(&file.path, &file.original, &bytes)?;
                    file.original = Some(bytes);
                    file.dirty = false;
                }
            }
        }
        if reload {
            if let Some(key) = selected {
                self.files.retain(|file| !equal_fold(&key, &file.key));
            } else {
                self.files.clear();
            }
        }
        Ok(())
    }

    /// Returns the ordered key/value pairs in the first matching section.
    pub fn section_pairs(&mut self, filename: Option<&[u32]>, section: &[u32]) -> RuntimeResult<Vec<(Vec<u32>, Vec<u32>)>> {
        let (key, path) = self.resolve_filename(filename)?;
        let Some(index) = self.file_index(&key, &path, false)? else { return Ok(Vec::new()) };
        Ok(self.files[index].sections.iter()
            .find(|entry| equal_fold(&entry.spelling, terminated(section)))
            .map(|entry| entry.values.iter().map(|value| (value.spelling.clone(), value.value.clone())).collect())
            .unwrap_or_default())
    }

    /// Returns every matching value in native multimap hash-chain order (newest first).
    pub fn get_all(&mut self, filename: Option<&[u32]>, section: &[u32], key: &[u32]) -> RuntimeResult<Vec<Vec<u32>>> {
        let (filename_key, path) = self.resolve_filename(filename)?;
        let Some(index) = self.file_index(&filename_key, &path, false)? else { return Ok(Vec::new()) };
        Ok(self.files[index].sections.iter()
            .find(|entry| equal_fold(&entry.spelling, terminated(section)))
            .map(|entry| entry.values.iter().rev().filter(|entry| equal_fold(&entry.spelling, terminated(key)))
                .map(|entry| entry.value.clone()).collect())
            .unwrap_or_default())
    }

    /// Prevents a selected cache entry from being written by later flushes.
    pub fn detach(&mut self, filename: Option<&[u32]>) -> RuntimeResult<()> {
        let (key, path) = self.resolve_filename(filename)?;
        if let Some(index) = self.file_index(&key, &path, true)? { self.files[index].no_save = true; }
        Ok(())
    }

    fn resolve_filename(&self, filename: Option<&[u32]>) -> RuntimeResult<(Vec<u32>, PathBuf)> {
        let Some(name) = filename else {
            return Ok((path_to_units(&self.system_ini)?, self.system_ini.clone()));
        };
        let name = terminated(name);
        let mut normalized = name.to_vec();
        // Match FConfigCacheIni's historical extension predicate, including its
        // second-to-last-character check for short dotted names.
        let len = normalized.len();
        if len < 5 || (normalized[len - 4] != b'.' as u32 && normalized[len - 5] != b'.' as u32) {
            normalized.extend(".ini".encode_utf16().map(u32::from));
        }
        let lower = lower_ascii(&normalized);
        if lower == "system.ini".encode_utf16().map(u32::from).collect::<Vec<_>>() {
            return Ok((path_to_units(&self.system_ini)?, self.system_ini.clone()));
        }
        if lower == "user.ini".encode_utf16().map(u32::from).collect::<Vec<_>>() {
            return Ok((path_to_units(&self.user_ini)?, self.user_ini.clone()));
        }
        let mut path_text = units_to_string(&normalized)?;
        // Native file I/O normalizes engine separators, but the config cache
        // retains its original filename key. Replacing ASCII preserves UTF-8.
        for byte in unsafe { path_text.as_bytes_mut() } {
            if *byte == b'\\' { *byte = b'/'; }
        }
        let candidate = PathBuf::from(path_text);
        let path = if candidate.is_absolute() || candidate.starts_with(&self.data_root) {
            candidate
        } else {
            self.data_root.join(candidate)
        };
        Ok((normalized, path))
    }

    fn file_index(&mut self, key: &[u32], path: &Path, create: bool) -> RuntimeResult<Option<usize>> {
        if let Some(index) = self.files.iter().position(|file| equal_fold(&file.key, key)) {
            return Ok(Some(index));
        }
        let original = read_config_bytes(path)?;
        if original.is_none() && !create { return Ok(None); }
        let content = original.as_deref().unwrap_or_default();
        // Never cache a malformed UTF-16 file as an empty document: a later
        // flush would otherwise destroy the unreadable original.
        let mut text: Vec<crate::ansi::Tchar> = crate::file_persistence::decode_ini_text(content)
            .ok_or_else(|| RuntimeError::new("Invalid UTF-16 configuration").with_context(&path.display().to_string()))?;
        text.push(0);
        let sections = std::cell::RefCell::new(Vec::new());
        let current = std::cell::Cell::new(None);
        // SAFETY: decoded text is owned, writable, and explicitly NUL-terminated.
        unsafe {
            crate::config_ini::parse_in_place(
                text.as_mut_ptr(),
                |name, length| -> Result<(), i32> {
                    // SAFETY: parser points into `text` and supplies the exact
                    // length of the current section token.
                    let spelling = unsafe { std::slice::from_raw_parts(name, length) }
                        .iter().map(|unit| *unit as u32).collect::<Vec<_>>();
                    let mut sections = sections.borrow_mut();
                    let index = sections.iter().position(|section: &ConfigSection| equal_fold(&section.spelling, &spelling));
                    let index = index.unwrap_or_else(|| {
                        sections.push(ConfigSection { spelling: spelling.clone(), values: Vec::new() });
                        sections.len() - 1
                    });
                    current.set(Some(index));
                    Ok(())
                },
                |key, key_len, value, value_len| -> Result<(), i32> {
                    if let Some(index) = current.get() {
                        let spelling = unsafe { std::slice::from_raw_parts(key, key_len) }
                            .iter().map(|unit| *unit as u32).collect();
                        let value = unsafe { std::slice::from_raw_parts(value, value_len) }
                            .iter().map(|unit| *unit as u32).collect();
                        sections.borrow_mut()[index].values.push(ConfigValue { spelling, value });
                    }
                    Ok(())
                },
            ).map_err(|code| RuntimeError::new("INI parser failed").with_context(&code.to_string()))?;
        }
        self.files.push(ConfigFile { key: key.to_vec(), path: path.to_path_buf(), sections: sections.into_inner(), dirty: false, no_save: false, original });
        Ok(Some(self.files.len() - 1))
    }

}

impl CoreAccess<'_> {
    /// Stages a changed-field update against a copy of the canonical System
    /// cache, publishes only that file, and commits the copy on success. An
    /// error leaves the original cache, including its dirty state, unchanged.
    /// The caller must serialize external writers: the filesystem check/rename
    /// race remains. No runtime object is updated by this method.
    pub fn config_update_system_staged(
        self,
        changes: &[(Vec<u32>, Vec<u32>, Vec<u32>)],
    ) -> RuntimeResult<()> {
        self.config_update_staged(None, changes)
    }

    /// Stages and publishes changed fields in the canonical User cache only.
    /// Failure leaves the canonical cache and its dirty state unchanged. This
    /// does not update live bindings. External writers must be serialized.
    pub fn config_update_user_staged(
        self,
        changes: &[(Vec<u32>, Vec<u32>, Vec<u32>)],
    ) -> RuntimeResult<()> {
        let user = [b'U' as u32, b's' as u32, b'e' as u32, b'r' as u32];
        self.config_update_staged(Some(&user), changes)
    }

    fn config_update_staged(
        self,
        filename: Option<&[u32]>,
        changes: &[(Vec<u32>, Vec<u32>, Vec<u32>)],
    ) -> RuntimeResult<()> {
        if changes.is_empty() { return Ok(()); }
        let config = unsafe { &mut *self.config_storage() };
        let mut staged = config.clone();
        let (key, path) = staged.resolve_filename(filename)?;
        for (section, name, value) in changes {
            staged.set(filename, section, name, value)?;
        }
        let index = staged.file_index(&key, &path, false)?
            .ok_or_else(|| RuntimeError::new("Configuration is unavailable"))?;
        if staged.files[index].no_save {
            return Err(RuntimeError::new("Configuration is detached"));
        }
        // Check even when the requested values equal the cache and no write is
        // needed: a stale cached value must not be reported as persisted.
        if read_config_bytes(&path)? != staged.files[index].original {
            return Err(RuntimeError::new("Configuration changed externally").with_context(&path.display().to_string()));
        }
        staged.flush(Some(&key), false)?;
        *config = staged;
        Ok(())
    }

    /// Creates an independent native-style config-cache session with the same
    /// path/language context and no shared cached files. Caller explicitly flushes it.
    pub fn new_config_cache(self) -> CoreConfig {
        unsafe { (&*self.config_storage()).fresh() }
    }

    /// Reports whether a canonical INI section exists, including an empty section.
    pub fn config_has_section_units(
        self,
        section: &[u32],
        filename: Option<&[u32]>,
    ) -> RuntimeResult<bool> {
        unsafe { (&mut *self.config_storage()).has_section(filename, section) }
    }
}

fn prepare_file(file: &ConfigFile) -> RuntimeResult<Option<Vec<u8>>> {
    if !file.dirty || file.no_save { return Ok(None); }
    let mut text = Vec::<crate::ansi::Tchar>::new();
    for section in &file.sections {
        text.push(b'[' as crate::ansi::Tchar);
        text.extend(section.spelling.iter().map(|unit| *unit as crate::ansi::Tchar));
        text.extend([b']' as crate::ansi::Tchar, 13, 10]);
        for pair in &section.values {
            text.extend(pair.spelling.iter().map(|unit| *unit as crate::ansi::Tchar));
            text.push(b'=' as crate::ansi::Tchar);
            text.extend(pair.value.iter().map(|unit| *unit as crate::ansi::Tchar));
            text.extend([13, 10]);
        }
        text.extend([13, 10]);
    }
    let bytes = crate::file_persistence::encode_ini_text(&text)
        .ok_or_else(|| RuntimeError::new("Invalid configuration text"))?;
    Ok(Some(bytes))
}

fn read_config_bytes(path: &Path) -> RuntimeResult<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(RuntimeError::new("Configuration path is not a regular file").with_context(&path.display().to_string()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("failed to inspect configuration", path, error)),
    }
    fs::read(path).map(Some).map_err(|error| io_error("failed to read configuration", path, error))
}

/// Rejects a final-component symlink or other non-regular destination and
/// preserves existing permission bits. Parent symlinks are not resolved or
/// secured here. Atomic replacement does not preserve ACLs, extended attributes,
/// ownership, or hard-link identity. A non-cooperating writer can race either
/// the path inspection or the final byte check and rename; callers must serialize
/// writers to obtain a true compare-and-swap guarantee.
fn publish_file(path: &Path, original: &Option<Vec<u8>>, bytes: &[u8]) -> RuntimeResult<()> {
    let check = || -> RuntimeResult<()> {
        if &read_config_bytes(path)? != original {
            return Err(RuntimeError::new("Configuration changed externally").with_context(&path.display().to_string()));
        }
        Ok(())
    };
    check()?;
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| io_error("failed to create configuration directory", parent, error))?;
    let permissions = if original.is_some() {
        Some(fs::metadata(path).map_err(|error| io_error("failed to inspect configuration", path, error))?.permissions())
    } else { None };
    let (temp_path, mut temp) = loop {
        let id = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".hp2-config-{}-{id}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        match options.open(&candidate) {
            Ok(file) => break (candidate, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error("failed to stage configuration", &candidate, error)),
        }
    };
    let result = (|| -> RuntimeResult<()> {
        temp.write_all(bytes).map_err(|error| io_error("failed to stage configuration", &temp_path, error))?;
        if let Some(permissions) = permissions {
            temp.set_permissions(permissions).map_err(|error| io_error("failed to set configuration permissions", &temp_path, error))?;
        }
        temp.sync_all().map_err(|error| io_error("failed to sync configuration", &temp_path, error))?;
        check()?;
        fs::rename(&temp_path, path).map_err(|error| io_error("failed to publish configuration", path, error))?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(&temp_path); }
    result
}

fn terminated(text: &[u32]) -> &[u32] { &text[..text.iter().position(|unit| *unit == 0).unwrap_or(text.len())] }
fn lower_ascii(text: &[u32]) -> Vec<u32> { text.iter().map(|unit| if (b'A' as u32..=b'Z' as u32).contains(unit) { unit + 32 } else { *unit }).collect() }
fn equal_fold(a: &[u32], b: &[u32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&left, &right)| fold(left) == fold(right))
}
fn fold(unit: u32) -> u32 {
    if char::from_u32(unit).is_some() {
        // SAFETY: valid Unicode scalar values are representable as host wint_t.
        unsafe { towlower(unit as Tchar) as u32 }
    } else {
        unit
    }
}

fn path_to_units(path: &Path) -> RuntimeResult<Vec<u32>> {
    path.to_str()
        .ok_or_else(|| RuntimeError::new("Invalid configuration filename"))
        .map(|text| text.chars().map(u32::from).collect())
}

fn units_to_string(units: &[u32]) -> RuntimeResult<String> {
    units.iter().copied().map(|unit| char::from_u32(unit).ok_or_else(|| RuntimeError::new("Invalid configuration filename"))).collect()
}
fn io_error(operation: &str, path: &Path, error: std::io::Error) -> RuntimeError {
    RuntimeError::new(operation).with_context(&format!("{}: {error}", path.display()))
}
