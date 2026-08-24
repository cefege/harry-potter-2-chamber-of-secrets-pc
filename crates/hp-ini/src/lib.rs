//! UE1 INI parse/merge/write (`FConfigFile`/`FConfigCacheIni` behavior
//! contract) plus the launcher path-resolution helper.
//!
//! Every quirk implemented here is pinned byte-for-byte by
//! `Tests/ConfigIniTests.cpp`; see that file's commentary for the rationale.
//! Semantics summary:
//! - lines split on `\r` or `\n` interchangeably; UTF-16LE+BOM input decoded
//! - section headers require a leading `[` AND trailing `]`, nothing after
//! - first `=` splits key/value; no trimming anywhere; no comment syntax
//! - quoted values lose exactly the outer quote pair (no escapes)
//! - duplicate keys are retained; lookups are case-insensitive last-wins
//! - case-variant section headers merge into the first-seen spelling
//! - writes are canonical CRLF with a blank line after every section

use std::fs;
use std::path::{Path, PathBuf};

/// Parse/save rejections. Each renders with its `settings.*`/`config.*`
/// reason-code prefix per Docs/REASON_CODES.md conventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IniError {
    /// File exists but cannot be read (permissions, directory...).
    Io(String),
    /// UTF-16LE body ended between a lone surrogate pair half.
    Utf16LoneSurrogate(u16),
}

impl std::fmt::Display for IniError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IniError::Io(what) => write!(f, "config.io_error: {what}"),
            IniError::Utf16LoneSurrogate(unit) => {
                write!(f, "config.utf16_lone_surrogate: 0x{unit:04X}")
            }
        }
    }
}

impl std::error::Error for IniError {}

/// One ordered key/value pair; duplicates are preserved deliberately.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pair {
    key: String,
    value: String,
}

/// One section: first-seen spelling plus insertion-ordered pairs.
#[derive(Debug, Clone, Default)]
struct Section {
    name: String,
    pairs: Vec<Pair>,
    dirty_new: bool,
}

/// A parsed config file (the `FConfigFile` analogue).
#[derive(Debug, Clone, Default)]
pub struct IniFile {
    sections: Vec<Section>,
    dirty: bool,
    /// `Detach` equivalent: never write this file back, even when dirty.
    no_save: bool,
}

impl IniFile {
    /// Parse from raw bytes applying every pinned quirk.
    pub fn parse(bytes: &[u8]) -> IniFile {
        let text = decode(bytes);
        let mut file = IniFile::default();
        let mut current: Option<usize> = None;
        for raw_line in split_lines(&text) {
            if raw_line.len() >= 2 && raw_line.starts_with('[') && raw_line.ends_with(']') {
                let name = &raw_line[1..raw_line.len() - 1];
                current = Some(file.section_index_for_merge(name));
                continue;
            }
            let Some(eq) = raw_line.find('=') else {
                continue; // no '=' -> silently dropped (incl. bare comments)
            };
            let Some(section) = current else {
                continue; // assignment before any section header -> dropped
            };
            let key = raw_line[..eq].to_string();
            let value = strip_outer_quotes(raw_line[eq + 1..].to_string());
            file.sections[section].pairs.push(Pair { key, value });
        }
        file
    }

    fn section_index_for_merge(&mut self, name: &str) -> usize {
        if let Some(index) = self
            .sections
            .iter()
            .position(|section| section.name.eq_ignore_ascii_case(name))
        {
            return index;
        }
        self.sections.push(Section {
            name: name.to_string(),
            ..Section::default()
        });
        self.sections.len() - 1
    }

    fn section_pos(&self, section: &str) -> Option<usize> {
        self.sections
            .iter()
            .position(|section_| section_.name.eq_ignore_ascii_case(section))
    }

    /// Case-insensitive newest-wins lookup (`Find`).
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let section = self.section_pos(section)?;
        self.get_in_section(section, key)
    }

    fn get_in_section(&self, section: usize, key: &str) -> Option<&str> {
        self.sections[section]
            .pairs
            .iter()
            .rev()
            .find(|pair| pair.key.eq_ignore_ascii_case(key))
            .map(|pair| pair.value.as_str())
    }

    /// Test/harness hook mirroring the C++ contract tests writing
    /// `File.Dirty = 1` directly before a forced rewrite.
    pub fn force_dirty(&mut self) {
        self.dirty = true;
    }

    /// All values for a key in file order (`MultiFind`).
    pub fn get_array<'a>(&'a self, section: &str, key: &str) -> Vec<&'a str> {
        let Some(section) = self.section_pos(section) else {
            return Vec::new();
        };
        self.sections[section]
            .pairs
            .iter()
            .filter(|pair| pair.key.eq_ignore_ascii_case(key))
            .map(|pair| pair.value.as_str())
            .collect()
    }

    /// `SetString`: case-insensitive lookup; a case-ONLY difference is a
    /// silent no-op (pinned migration-critical quirk); otherwise the newest
    /// matching entry updates in place, or a fresh pair appends.
    pub fn set_string(&mut self, section: &str, key: &str, value: &str) {
        let index = self.section_index_for_merge(section);
        let existing = self.sections[index]
            .pairs
            .iter()
            .rev()
            .position(|pair| pair.key.eq_ignore_ascii_case(key))
            .map(|reverse_position| self.sections[index].pairs.len() - 1 - reverse_position);
        match existing {
            Some(pair_index) => {
                let pair = &self.sections[index].pairs[pair_index];
                if pair.key == key && pair.value == value {
                    return; // byte-identical: clean
                }
                if pair.value.eq_ignore_ascii_case(value) && !pair.value.is_empty()
                    || pair.value == value && pair.key != key
                {
                    // Case-only value change (or identical value under a
                    // case-variant key spelling): pinned silent no-op.
                    return;
                }
                self.sections[index].pairs[pair_index].value = value.to_string();
                self.dirty = true;
            }
            None => {
                self.sections[index].pairs.push(Pair {
                    key: key.to_string(),
                    value: value.to_string(),
                });
                self.dirty = true;
            }
        }
    }

    /// Typed writer: `%i` formatting.
    pub fn set_int(&mut self, section: &str, key: &str, value: i32) {
        self.set_string_typed(section, key, value.to_string());
    }

    /// Typed writer: `%f` formatting (six decimals, C locale).
    pub fn set_float(&mut self, section: &str, key: &str, value: f32) {
        self.set_string_typed(section, key, format_c_float(value));
    }

    /// Typed writer: literal `True`/`False`.
    pub fn set_bool(&mut self, section: &str, key: &str, value: bool) {
        self.set_string_typed(
            section,
            key,
            if value { "True" } else { "False" }.to_string(),
        );
    }

    fn set_string_typed(&mut self, section: &str, key: &str, rendered: String) {
        let index = self.section_index_for_merge(section);
        self.sections[index].pairs.push(Pair {
            key: key.to_string(),
            value: rendered,
        });
        self.dirty = true;
    }

    /// Clears an existing section (it survives as a bare header); a missing
    /// section is a no-op without dirtying.
    pub fn empty_section(&mut self, section: &str) {
        let Some(index) = self.section_pos(section) else {
            return;
        };
        if !self.sections[index].pairs.is_empty() {
            self.sections[index].pairs.clear();
            self.sections[index].dirty_new = false;
            self.dirty = true;
        }
    }

    /// Never write this file back (`Detach`/NoSave).
    pub fn detach(&mut self) {
        self.no_save = true;
    }

    /// Canonical serialization: `[Name]\r\nKey=Value\r\n` per pair, blank
    /// line after every section. Returns `None` for a clean or NoSave file,
    /// and for a dirty-but-empty config (save refuses empty strings).
    pub fn render_if_dirty(&self) -> Option<Vec<u8>> {
        if self.no_save || !self.dirty {
            return None;
        }
        if self.sections.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        for section in &self.sections {
            out.extend_from_slice(format!("[{}]\r\n", section.name).as_bytes());
            for pair in &section.pairs {
                out.extend_from_slice(format!("{}={}\r\n", pair.key, pair.value).as_bytes());
            }
            out.extend_from_slice(b"\r\n");
        }
        if let Ok(text) = std::str::from_utf8(&out)
            && text.chars().any(|c| c as u32 > 0xFF)
        {
            let mut wide = Vec::with_capacity(out.len() * 2 + 2);
            wide.extend_from_slice(&[0xFF, 0xFE]); // BOM
            for unit in text.encode_utf16() {
                wide.extend_from_slice(&unit.to_le_bytes());
            }
            return Some(wide);
        }
        Some(out)
    }

    /// Write to disk when dirty; returns whether a write happened
    /// (`File.Write` returns 1 for clean-no-op too).
    pub fn write(&mut self, path: &Path) -> Result<bool, IniError> {
        let Some(bytes) = self.render_if_dirty() else {
            return Ok(true);
        };
        fs::write(path, &bytes).map_err(|error| IniError::Io(error.to_string()))?;
        self.dirty = false;
        Ok(true)
    }

    /// Number of sections (`File.Num()`).
    pub fn len(&self) -> usize {
        self.sections.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

fn decode(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    // ANSI path: the engine's host code page is ASCII-superset; treat bytes
    // as Latin-1 so every byte round-trips through the Unicode String.
    bytes.iter().map(|byte| *byte as char).collect()
}

fn split_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split(['\r', '\n'])
}

fn strip_outer_quotes(value: String) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].to_string()
    } else {
        value
    }
}

/// `%f` in C locale: six fractional digits, `-` sign only when negative.
fn format_c_float(value: f32) -> String {
    format!("{value:.6}")
}

// ---------------------------------------------------------------- typed reads

/// `appAtoi` == `wcstol base 10`: skip whitespace, optional sign, digits.
pub fn atoi_quirk(text: &str) -> i32 {
    let trimmed = text.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut sign = 1i64;
    if matches!(chars.peek(), Some('+')) {
        chars.next();
    } else if matches!(chars.peek(), Some('-')) {
        sign = -1;
        chars.next();
    }
    let mut acc: i64 = 0;
    while let Some(digit) = chars.peek().and_then(|c| c.to_digit(10)) {
        acc = acc * 10 + digit as i64;
        if acc > i32::MAX as i64 {
            acc = i32::MAX as i64; // wcstol clamps; fixtures stay far below
        }
        chars.next();
    }
    (sign * acc) as i32
}

/// `atof` in the C locale: decimal point only, exponent supported, stops at
/// the first non-parseable character.
pub fn atof_quirk(text: &str) -> f32 {
    let trimmed = text.trim_start();
    let bytes = trimmed.as_bytes();
    let mut end = 0usize;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    let mut seen_digit = false;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
        seen_digit = true;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        let dot = end;
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
            seen_digit = true;
        }
        if !seen_digit {
            end = dot; // "." alone parses nothing
        }
    }
    if seen_digit && end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut exp_end = end + 1;
        if exp_end < bytes.len() && (bytes[exp_end] == b'+' || bytes[exp_end] == b'-') {
            exp_end += 1;
        }
        let exp_digits_start = exp_end;
        while exp_end < bytes.len() && bytes[exp_end].is_ascii_digit() {
            exp_end += 1;
        }
        if exp_end > exp_digits_start {
            end = exp_end;
        }
    }
    if !seen_digit {
        return 0.0;
    }
    trimmed[..end].parse::<f32>().unwrap_or(0.0)
}

impl IniFile {
    /// `GetInt`: returns `Some(parsed)` iff the key exists (garbage parses
    /// as 0 but still counts as found).
    pub fn get_int(&self, section: &str, key: &str) -> Option<i32> {
        self.get(section, key).map(atoi_quirk)
    }

    /// `GetFloat`: C-locale `atof` over an existing key's text.
    pub fn get_float(&self, section: &str, key: &str) -> Option<f32> {
        self.get(section, key).map(atof_quirk)
    }

    /// `GetBool`: case-insensitive literal "True", else `atoi(text)==1`.
    pub fn get_bool(&self, section: &str, key: &str) -> Option<bool> {
        self.get(section, key)
            .map(|text| text.eq_ignore_ascii_case("True") || atoi_quirk(text) == 1)
    }
}

// ------------------------------------------------------------- merged set

/// Default ini overlaid by an optional user ini (user wins per-key lookups;
/// arrays concatenate default-then-user in file order).
#[derive(Debug, Clone, Default)]
pub struct IniSet {
    default: IniFile,
    user: Option<IniFile>,
}

/// Read a default ini plus optional user overlay. A missing default file is
/// an error (`data.prototype_missing`-style loudness); a missing user file
/// simply means no overlay, mirroring `FConfigFile::Read`.
pub fn load_pair(default_ini: &Path, user_ini: Option<&Path>) -> Result<IniSet, IniError> {
    let default_bytes = fs::read(default_ini)
        .map_err(|error| IniError::Io(format!("{}: {error}", default_ini.display())))?;
    let user = match user_ini {
        Some(path) => match fs::read(path) {
            Ok(bytes) => Some(IniFile::parse(&bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(IniError::Io(format!("{}: {error}", path.display())));
            }
        },
        None => None,
    };
    Ok(IniSet {
        default: IniFile::parse(&default_bytes),
        user,
    })
}

impl IniSet {
    /// User overlay wins; falls back to the default file.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        if let Some(user) = &self.user
            && let Some(value) = user.get(section, key)
        {
            return Some(value);
        }
        self.default.get(section, key)
    }

    /// Default-file matches first, then user-file matches (file order).
    pub fn get_array<'a>(&'a self, section: &str, key: &str) -> Vec<&'a str> {
        let mut values = self.default.get_array(section, key);
        if let Some(user) = &self.user {
            values.extend(user.get_array(section, key));
        }
        values
    }

    /// Resolve `Paths=` entries against `<data_root>/System` — the engine
    /// process's working directory, which is why stock entries read
    /// `../Maps/*.unr` — then normalize lexically. When neither file
    /// carries any Paths entry, append the stock defaults
    /// `System Maps Textures Sounds Music`.
    pub fn paths(&self, data_root: &Path) -> Vec<PathBuf> {
        let mut resolved: Vec<PathBuf> = Vec::new();
        for entry in self.get_array("Paths", "Paths") {
            let candidate = PathBuf::from(entry.replace('\\', "/"));
            let joined = if candidate.is_absolute() {
                normalize_lexical(&candidate)
            } else {
                normalize_lexical(&data_root.join("System").join(candidate))
            };
            if !resolved.contains(&joined) {
                resolved.push(joined);
            }
        }
        if resolved.is_empty() {
            for dir in ["System", "Maps", "Textures", "Sounds", "Music"] {
                resolved.push(data_root.join(dir));
            }
        }
        resolved
    }
}

/// `FConfigCacheIni` filename resolution: append `.ini` unless the name is
/// five or more characters AND carries a dot within its last five
/// characters (either `Len-4` or `Len-5` counts); otherwise use verbatim.
pub fn resolve_ini_name(name: &str) -> String {
    let bytes = name.as_bytes();
    let len = bytes.len();
    let dotted = len >= 5 && (bytes[len - 4] == b'.' || bytes[len - 5] == b'.');
    if dotted {
        name.to_string()
    } else {
        format!("{name}.ini")
    }
}

/// Lexical normalization: collapse `.` and `name/..` pairs without touching
/// the filesystem (symlink-free by construction; engine paths are data-root
/// relative).
fn normalize_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut stack: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match stack.last() {
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                _ => {
                    // Nothing normal to pop (root, leading, or stacked `..`):
                    // keep the segment literal rather than touching the FS.
                    stack.push(component);
                }
            },
            other => stack.push(other),
        }
    }
    stack.into_iter().collect()
}
