//! INI text helpers over `hp-ini` for the launcher store's strict
//! full-value parsers (`Integer`, `Number`, `Boolean` from
//! `HP2LauncherStore.cpp`).
//!
//! The engine's typed readers differ from the launcher's: the launcher
//! requires whole-value parses and treats unknown spellings as absent so
//! callers fall back to model defaults. These helpers pin that contract.

pub use hp_ini::IniFile as IniDoc;

/// Parses an INI document applying every pinned hp-ini quirk.
pub fn parse_ini(bytes: &[u8]) -> IniDoc {
    IniDoc::parse(bytes)
}

/// Case-insensitive newest-wins raw lookup.
pub fn get<'a>(doc: &'a IniDoc, section: &str, key: &str) -> Option<&'a str> {
    doc.get(section, key)
}

/// `Boolean`: true/on/yes/1 vs false/off/no/0, case-insensitive; anything
/// else is unrecognized.
pub fn parse_boolean(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

/// Lookup + [`parse_boolean`].
pub fn get_bool(doc: &IniDoc, section: &str, key: &str) -> Option<bool> {
    get(doc, section, key).and_then(parse_boolean)
}

/// `Integer`: optional sign then digits only, whole value consumed.
pub fn parse_integer(text: &str) -> Option<i32> {
    let text = text.trim();
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i32>().ok()
}

/// Lookup + [`parse_number`].
pub fn get_number(doc: &IniDoc, section: &str, key: &str) -> Option<f64> {
    parse_number(get(doc, section, key)?)
}

/// Lookup + [`parse_integer`] with a range check.
pub fn get_integer_ranged(
    doc: &IniDoc,
    section: &str,
    key: &str,
    range: std::ops::RangeInclusive<i32>,
) -> Option<i32> {
    parse_integer(get(doc, section, key)?).filter(|value| range.contains(value))
}

/// Plain lookup + strict integer parse.
pub fn get_integer(doc: &IniDoc, section: &str, key: &str) -> Option<i32> {
    parse_integer(get(doc, section, key)?)
}

/// `Number`: C-locale whole-value float parse (accepts inf/nan spellings
/// like `strtod`; callers range-check).
pub fn parse_number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok()
}

/// Lookup + [`parse_number`] with an inclusive range check.
pub fn get_ranged_float(
    doc: &IniDoc,
    section: &str,
    key: &str,
    minimum: f64,
    maximum: f64,
) -> Option<f64> {
    parse_number(get(doc, section, key)?)
        .filter(|value| value.is_finite() && (minimum..=maximum).contains(value))
}

/// Lookup + discrete membership check; returns the accepted value itself.
pub fn get_discrete_float(doc: &IniDoc, section: &str, key: &str, accepted: &[f64]) -> Option<f64> {
    let parsed = parse_number(get(doc, section, key)?)?;
    accepted.iter().copied().find(|value| *value == parsed)
}

/// Lookup + strict integer + discrete membership check.
pub fn get_discrete_integer(
    doc: &IniDoc,
    section: &str,
    key: &str,
    accepted: &[i32],
) -> Option<i32> {
    let parsed = parse_integer(get(doc, section, key)?)?;
    accepted.iter().copied().find(|value| *value == parsed)
}

/// Canonical double rendering used by commits: shortest round-trip form
/// (`DoubleText`, e.g. `1`, `0.4`, `0.85`, `0.53`).
pub fn double_text(value: f64) -> String {
    format!("{value}")
}

/// `BoolText`: canonical persisted booleans.
pub fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

// ------------------------------------------------- launcher text documents

/// Byte-preserving launcher INI encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IniEncoding {
    /// No BOM; strict UTF-8 bytes.
    Utf8,
    /// `FF FE` BOM, little-endian UTF-16 units.
    Utf16Le,
    /// `FE FF` BOM, big-endian UTF-16 units.
    Utf16Be,
}

#[derive(Debug, Clone)]
struct DocLine {
    text: String,
    terminator: &'static str,
}

/// A launcher-owned INI document that preserves unrelated content exactly:
/// comments, malformed lines, inline comments, per-line newline style, and
/// the file encoding (`HP2LauncherStore.cpp` edits documents in place; the
/// engine's canonical rewriter in [`IniFile`] would destroy them).
///
/// Lookup semantics mirror the loader: case-insensitive section headers
/// require a leading `[` and trailing `]`, pairs are last-wins on the
/// trimmed key, and values keep their raw text (numeric readers apply
/// `strtod`/`atoi` longest-prefix parsing).
#[derive(Debug, Clone)]
pub struct LauncherDoc {
    lines: Vec<DocLine>,
    encoding: IniEncoding,
}

impl LauncherDoc {
    /// Decodes raw bytes (UTF-16 LE/BE by BOM, else strict UTF-8). Odd
    /// UTF-16 unit counts, lone surrogates, and invalid UTF-8 are loud
    /// errors so callers can fall back to immutable templates.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let (text, encoding) = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
            (
                decode_utf16(&bytes[2..], true)?,
                IniEncoding::Utf16Le,
            )
        } else if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
            (
                decode_utf16(&bytes[2..], false)?,
                IniEncoding::Utf16Be,
            )
        } else {
            (
                std::str::from_utf8(bytes)
                    .map_err(|_| "document is not valid UTF-8".to_string())?
                    .to_string(),
                IniEncoding::Utf8,
            )
        };
        Ok(Self {
            lines: split_doc_lines(&text),
            encoding,
        })
    }

    fn header_name(line: &str) -> Option<&str> {
        let trimmed = line.trim();
        (trimmed.len() >= 2 && trimmed.starts_with('[') && trimmed.ends_with(']'))
            .then(|| &trimmed[1..trimmed.len() - 1])
    }

    fn pair_key(line: &str) -> Option<&str> {
        line.split_once('=').map(|(key, _)| key)
    }

    fn matches_pair(&self, line: &DocLine, section: &str, key: &str) -> bool {
        Self::pair_key(&line.text).is_some_and(|pair_key| pair_key.trim().eq_ignore_ascii_case(key))
            && self.in_section(line, section)
    }

    fn in_section(&self, line: &DocLine, section: &str) -> bool {
        // Walk backwards to the nearest header; a malformed pseudo-header
        // like "[Engine.GameEngine" never opens a section.
        let mut seen = false;
        for candidate in self.lines.iter().rev() {
            if candidate.text == line.text && candidate.terminator == line.terminator {
                if seen {
                    break;
                }
                seen = true;
                continue;
            }
            if let Some(name) = Self::header_name(&candidate.text) {
                return name.eq_ignore_ascii_case(section);
            }
        }
        false
    }

    /// Case-insensitive last-wins raw value lookup.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        self.lines
            .iter()
            .rev()
            .find(|line| self.matches_pair(line, section, key))
            .and_then(|line| line.text.split_once('='))
            .map(|(_, value)| value)
    }

    /// Rewrites the final occurrence of `section/key` in place, preserving
    /// the key text and anything after the value token (inline comments).
    /// A missing pair is appended inside the matching section group, or
    /// under a fresh header at the end of the document.
    pub fn set_value(&mut self, section: &str, key: &str, value: &str) {
        if let Some(index) = self
            .lines
            .iter()
            .rposition(|line| self.matches_pair(line, section, key))
        {
            let line = &mut self.lines[index];
            let (before, rest) = line.text.split_once('=').expect("matched pair");
            line.text = format!(
                "{before}={}",
                replace_value_token(rest, value)
            );
            return;
        }
        self.append_pair(section, key, value);
    }

    fn append_pair(&mut self, section: &str, key: &str, value: &str) {
        // End of the LAST case-variant group for this section.
        let mut insert_at = None;
        for index in 0..self.lines.len() {
            if Self::header_name(&self.lines[index].text)
                .is_some_and(|name| name.eq_ignore_ascii_case(section))
            {
                let mut end = index + 1;
                while end < self.lines.len() && Self::header_name(&self.lines[end].text).is_none() {
                    end += 1;
                }
                insert_at = Some(end);
            }
        }
        match insert_at {
            Some(at) => {
                let terminator = insertion_terminator(&self.lines, at);
                self.lines.insert(
                    at,
                    DocLine {
                        text: format!("{key}={value}"),
                        terminator,
                    },
                );
            }
            None => {
                let terminator = dominant_terminator(&self.lines);
                self.lines.push(DocLine {
                    text: format!("[{section}]"),
                    terminator,
                });
                self.lines.push(DocLine {
                    text: format!("{key}={value}"),
                    terminator,
                });
            }
        }
    }

    /// Re-encodes the document in its original encoding (BOM included for
    /// the UTF-16 variants).
    pub fn render(&self) -> Vec<u8> {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(&line.text);
            text.push_str(line.terminator);
        }
        match self.encoding {
            IniEncoding::Utf8 => text.into_bytes(),
            IniEncoding::Utf16Le => encode_utf16(&text, true),
            IniEncoding::Utf16Be => encode_utf16(&text, false),
        }
    }
}

fn decode_utf16(bytes: &[u8], little: bool) -> Result<String, String> {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let unit = if little {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        };
        units.push(unit);
    }
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 body ends between units".to_string());
    }
    String::from_utf16(&units).map_err(|_| "UTF-16 body contains a lone surrogate".to_string())
}

fn encode_utf16(text: &str, little: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    out.extend_from_slice(if little { &[0xFF, 0xFE] } else { &[0xFE, 0xFF] });
    for unit in text.encode_utf16() {
        if little {
            out.extend_from_slice(&unit.to_le_bytes());
        } else {
            out.extend_from_slice(&unit.to_be_bytes());
        }
    }
    out
}

fn split_doc_lines(text: &str) -> Vec<DocLine> {
    let mut lines = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let mut end = index;
            let terminator: &'static str = if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
                "\r\n"
            } else {
                "\n"
            };
            lines.push(DocLine {
                text: text[start..end].to_string(),
                terminator,
            });
            start = index + 1;
        }
        index += 1;
    }
    if start < text.len() {
        lines.push(DocLine {
            text: text[start..].to_string(),
            terminator: "",
        });
    }
    lines
}

fn replace_value_token(rest: &str, value: &str) -> String {
    let lead = rest.len() - rest.trim_start().len();
    let after = &rest[lead..];
    let token_end = after.find(char::is_whitespace).unwrap_or(after.len());
    format!("{}{}{}", &rest[..lead], value, &after[token_end..])
}

fn insertion_terminator(lines: &[DocLine], at: usize) -> &'static str {
    match lines[..at].iter().rev().find(|line| !line.terminator.is_empty()) {
        Some(line) => line.terminator,
        None => "\r\n",
    }
}

fn dominant_terminator(lines: &[DocLine]) -> &'static str {
    insertion_terminator(lines, lines.len())
}

/// `strtod`-style longest-prefix float parse over a trimmed value
/// (`" 144 ; comment"` parses as `144`).
pub fn prefix_number(text: &str) -> Option<f64> {
    let rest = text.trim_start();
    let bytes = rest.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
    let mut digits = false;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
        digits = true;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        let mut fraction = end + 1;
        while fraction < bytes.len() && bytes[fraction].is_ascii_digit() {
            fraction += 1;
            digits = true;
        }
        if digits {
            end = fraction;
        }
    }
    if !digits {
        return None;
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut exponent = end + 1;
        if matches!(bytes.get(exponent), Some(b'+' | b'-')) {
            exponent += 1;
        }
        let exponent_start = exponent;
        while exponent < bytes.len() && bytes[exponent].is_ascii_digit() {
            exponent += 1;
        }
        if exponent > exponent_start {
            end = exponent;
        }
    }
    rest[..end].parse::<f64>().ok()
}

/// `atoi`-style longest-prefix integer parse over a trimmed value.
pub fn prefix_integer(text: &str) -> Option<i64> {
    let rest = text.trim_start();
    let bytes = rest.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+') | Some(b'-')));
    let start = end;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end == start {
        return None;
    }
    rest[..end].parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_spellings_match_the_native_store() {
        assert_eq!(parse_boolean("True"), Some(true));
        assert_eq!(parse_boolean("on"), Some(true));
        assert_eq!(parse_boolean(" YES "), Some(true));
        assert_eq!(parse_boolean("No"), Some(false));
        assert_eq!(parse_boolean("OFF"), Some(false));
        assert_eq!(parse_boolean("perhaps"), None);
        assert_eq!(parse_boolean(""), None);
    }

    #[test]
    fn number_rejects_partial_and_garbage_parses() {
        assert_eq!(parse_number("60.000000"), Some(60.0));
        assert_eq!(parse_number("nan").map(f64::is_nan), Some(true));
        assert_eq!(parse_number("invalid"), None);
        assert_eq!(parse_number("12abc"), None);
        assert_eq!(parse_number("  0.75  "), Some(0.75));
    }

    #[test]
    fn integer_is_strict_and_signed() {
        assert_eq!(parse_integer("+42"), Some(42));
        assert_eq!(parse_integer("-7"), Some(-7));
        assert_eq!(parse_integer("12x"), None);
        assert_eq!(parse_integer(""), None);
        assert_eq!(parse_integer("99999999999"), None);
    }
}
