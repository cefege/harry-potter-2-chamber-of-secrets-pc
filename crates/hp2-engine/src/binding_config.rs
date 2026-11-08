//! Narrow, offline adapter for existing `[Engine.Input]` User.ini key bindings.
//!
//! This does not update a live `UInput` or its config cache. The caller must
//! serialize all config writers and must not use it while an engine with a
//! cached copy can later overwrite the file. The in-game UI must not use this
//! adapter while the engine is running. It is not runtime rebinding.
use crate::binding_foundation::{self, BindingError};
use hp2_core::{core_access::RuntimeError, CoreConfig};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigBindingError {
    Config(RuntimeError),
    Binding(BindingError),
    InvalidPath,
    InvalidKey,
    MissingOrDuplicateKey,
    InvalidText,
}
impl From<RuntimeError> for ConfigBindingError {
    fn from(value: RuntimeError) -> Self { Self::Config(value) }
}
impl From<BindingError> for ConfigBindingError {
    fn from(value: BindingError) -> Self { Self::Binding(value) }
}

fn units(s: &str) -> Vec<u32> { s.chars().map(u32::from).collect() }
fn string(units: &[u32]) -> Result<String, ConfigBindingError> {
    units.iter().copied().map(char::from_u32).collect::<Option<String>>().ok_or(ConfigBindingError::InvalidText)
}

/// An isolated Core config cache. `known_keys` must come from canonical
/// EInputKey metadata, not from menu-visible actions. Existing unique keys only
/// are editable: absent files/keys and duplicate keys are rejected instead of
/// guessing how to merge defaults or ambiguous entries.
pub struct BindingConfig {
    config: CoreConfig,
    user_path: Vec<u32>,
    known_keys: Vec<String>,
}
impl BindingConfig {
    pub fn open(data_root: &Path, system_ini: &Path, user_ini: &Path, known_keys: &[&str]) -> Result<Self, ConfigBindingError> {
        if !user_ini.is_absolute() || !user_ini.is_file() { return Err(ConfigBindingError::InvalidPath); }
        let user_path = units(user_ini.to_str().ok_or(ConfigBindingError::InvalidPath)?);
        let valid = |key: &str| !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
        if known_keys.iter().any(|key| !valid(key) || key.eq_ignore_ascii_case("Aliases")) {
            return Err(ConfigBindingError::InvalidKey);
        }
        Ok(Self {
            config: CoreConfig::new(PathBuf::from(data_root), PathBuf::from(system_ini), PathBuf::from(user_ini)),
            user_path,
            known_keys: known_keys.iter().map(|key| (*key).to_owned()).collect(),
        })
    }

    fn key(&self, key: &str) -> Result<Vec<u32>, ConfigBindingError> {
        if !self.known_keys.iter().any(|name| name.eq_ignore_ascii_case(key)) { return Err(ConfigBindingError::InvalidKey); }
        Ok(units(key))
    }

    /// Returns the exact current binding in this cache; does not resolve aliases.
    pub fn read(&mut self, key: &str) -> Result<String, ConfigBindingError> {
        let key = self.key(key)?;
        let mut values = self.config.get_all(Some(&units("User.ini")), &units("Engine.Input"), &key)?;
        if values.len() != 1 { return Err(ConfigBindingError::MissingOrDuplicateKey); }
        string(&values.remove(0))
    }

    /// Stage only complete commands on one existing key. Failures leave it unchanged.
    pub fn edit(&mut self, key: &str, remove: &[&str], add: &[&str]) -> Result<bool, ConfigBindingError> {
        let old = self.read(key)?;
        let mut new = old.clone();
        for command in remove { new = binding_foundation::remove(&new, command)?; }
        for command in add { new = binding_foundation::add(&new, command)?; }
        if old == new { return Ok(false); }
        let name = self.key(key)?;
        self.config.set(Some(&units("User.ini")), &units("Engine.Input"), &name, &units(&new))?;
        Ok(true)
    }

    /// Publish only this cache's User.ini. The cache is consumed even on error.
    /// Core checks for external file changes but cannot prevent a final rename
    /// race; writers must be serialized. Core preserves values, not INI comments
    /// or formatting, and does not preserve ACLs or extended attributes.
    pub fn commit(mut self) -> Result<(), ConfigBindingError> {
        self.config.flush(Some(&self.user_path), false)?;
        Ok(())
    }
}
