//! Byte-preserving launcher INI documents and strict value parsers,
//! mirroring the document model in
//! `HarryPotter2/Unreal/SDLLaunch/Src/HP2LauncherStore.cpp`
//! (`Decode`/`Encode`/`Section`/`Key`/`Get`/`Set`/`Erase` and the
//! `Integer`/`Number`/`Boolean` readers).
//!
//! The engine's canonical rewriter in `hp-ini` would destroy comments,
//! malformed lines, per-line newline styles, and file encodings; the
//! launcher edits documents in place, so every unrelated byte must
//! survive a commit untouched. [`LauncherDoc`] pins that contract.

/// Strict whole-value float parse (`Number`: C locale, finite, entire
/// value consumed; callers range-check).
pub fn parse_number(text: &str) -> Option<f64> {
    text.trim().parse::<f64>().ok()
}

/// Lookup + [`parse_number`].
pub fn get_number(doc: &LauncherDoc, section: &str, key: &str) -> Option<f64> {
    parse_number(doc.get(section, key)?)
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
pub fn get_bool(doc: &LauncherDoc, section: &str, key: &str) -> Option<bool> {
    parse_boolean(doc.get(section, key)?)
}

/// `Integer`: optional sign then digits only, whole value consumed.
pub fn parse_integer(text: &str) -> Option<i32> {
    text.trim().parse::<i32>().ok()
}

/// Lookup + strict integer parse.
pub fn get_integer(doc: &LauncherDoc, section: &str, key: &str) -> Option<i32> {
    parse_integer(doc.get(section, key)?)
}

/// Lookup + [`parse_number`] with an inclusive range check (`LoadNumber`).
pub fn get_ranged_float(
    doc: &LauncherDoc,
    section: &str,
    key: &str,
    minimum: f64,
    maximum: f64,
) -> Option<f64> {
    let parsed = get_number(doc, section, key)?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

/// Lookup + discrete membership check; returns the accepted value itself
/// (`LoadDiscreteNumber`).
pub fn get_discrete_float(doc: &LauncherDoc, section: &str, key: &str, accepted: &[f64]) -> Option<f64> {
    let parsed = parse_number(doc.get(section, key)?)?;
    accepted.iter().copied().find(|value| *value == parsed)
}

/// Lookup + integral discrete membership check (`LoadDiscreteInteger`).
pub fn get_discrete_integer(
    doc: &LauncherDoc,
    section: &str,
    key: &str,
    accepted: &[i32],
) -> Option<i32> {
    let parsed = parse_number(doc.get(section, key)?)?;
    let integral = parsed == parsed.trunc()
        && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&parsed);
    let discrete = integral.then_some(parsed as i32)?;
    accepted.iter().copied().find(|value| *value == discrete)
}

/// Canonical double rendering used by commits (`DoubleText`, e.g. `1`,
/// `0.4`, `0.85`, `0.53`; every accepted discrete choice has an exact
/// short decimal form, so shortest round-trip matches `%g` precision 15).
pub fn double_text(value: f64) -> String {
    format!("{value}")
}

/// `BoolText`: canonical persisted booleans.
pub fn bool_text(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

/// Free-function form of [`LauncherDoc::get`] used by the reader helpers.
pub fn get<'a>(doc: &'a LauncherDoc, section: &str, key: &str) -> Option<&'a str> {
    doc.get(section, key)
}

// ------------------------------------------------- launcher text documents

/// Byte-preserving launcher INI encoding (`Encoding`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IniEncoding {
    /// No BOM; strict UTF-8 bytes.
    Utf8,
    /// `EF BB BF` BOM followed by UTF-8 bytes.
    Utf8Bom,
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

/// One parsed `key=value` row: the trimmed key, the comment-stripped
/// trailing-space-trimmed value token, and that token's span inside the
/// line so in-place rewrites can preserve everything around it.
struct Pair<'a> {
    key: &'a str,
    value: &'a str,
    value_span: std::ops::Range<usize>,
}

fn is_space_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\x0b' | b'\x0c' | b'\r' | b'\n')
}

fn is_space_char(character: char) -> bool {
    matches!(
        character,
        ' ' | '\t' | '\u{0b}' | '\u{0c}' | '\r' | '\n'
    )
}

/// Ports `Key`: rejects blank and comment lines, requires a non-empty
/// trimmed key before the first `=`, and slices the value token at the
/// first whitespace-preceded `;` or `#`.
fn parse_pair(line: &str) -> Option<Pair<'_>> {
    let bytes = line.as_bytes();
    let mut begin = 0;
    while begin < bytes.len() && is_space_byte(bytes[begin]) {
        begin += 1;
    }
    if begin == bytes.len() || bytes[begin] == b';' || bytes[begin] == b'#' {
        return None;
    }
    let equals = begin + line[begin..].bytes().position(|byte| byte == b'=')?;
    let key = line[begin..equals].trim_matches(is_space_char);
    if key.is_empty() {
        return None;
    }
    let mut value_begin = equals + 1;
    while value_begin < bytes.len() && is_space_byte(bytes[value_begin]) {
        value_begin += 1;
    }
    let mut comment = bytes.len();
    for index in value_begin..bytes.len() {
        if (bytes[index] == b';' || bytes[index] == b'#')
            && (index == value_begin || is_space_byte(bytes[index - 1]))
        {
            comment = index;
            break;
        }
    }
    let mut value_end = comment;
    while value_end > value_begin && is_space_byte(bytes[value_end - 1]) {
        value_end -= 1;
    }
    Some(Pair {
        key,
        value: &line[value_begin..value_end],
        value_span: value_begin..value_end,
    })
}

/// A launcher-owned INI document that preserves unrelated content exactly:
/// comments, malformed lines, inline comments, per-line newline style, and
/// the file encoding (`HP2LauncherStore.cpp` edits documents in place).
///
/// Lookup semantics mirror the loader: valid section headers require a
/// non-empty bracketed name without nested brackets, pairs are last-wins
/// on the case-insensitive trimmed key, and values are returned with any
/// inline comment stripped ([`Key`] semantics), so the strict readers see
/// exactly the token the native store would parse.
#[derive(Debug, Clone)]
pub struct LauncherDoc {
    lines: Vec<DocLine>,
    encoding: IniEncoding,
    newline: &'static str,
}

impl LauncherDoc {
    /// Decodes raw bytes (UTF-16 LE/BE by BOM, UTF-8 BOM, else strict
    /// UTF-8). Odd UTF-16 unit counts, lone surrogates, and invalid UTF-8
    /// are loud errors so callers can fall back to immutable templates.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let (text, encoding) = if bytes.starts_with(&[0xFF, 0xFE]) {
            (
                decode_utf16(&bytes[2..], true)?,
                IniEncoding::Utf16Le,
            )
        } else if bytes.starts_with(&[0xFE, 0xFF]) {
            (
                decode_utf16(&bytes[2..], false)?,
                IniEncoding::Utf16Be,
            )
        } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            (
                std::str::from_utf8(&bytes[3..])
                    .map_err(|_| "document is not valid UTF-8".to_string())?
                    .to_string(),
                IniEncoding::Utf8Bom,
            )
        } else {
            (
                std::str::from_utf8(bytes)
                    .map_err(|_| "document is not valid UTF-8".to_string())?
                    .to_string(),
                IniEncoding::Utf8,
            )
        };
        let mut document = Self {
            lines: split_doc_lines(&text),
            encoding,
            newline: "\n",
        };
        document.newline = dominant_terminator(&document.lines);
        Ok(document)
    }

    /// Ports `Section`: a valid header trims to `[name]` with a non-empty
    /// name free of nested brackets.
    fn header_name(line: &str) -> Option<&str> {
        let trimmed = line.trim_matches(is_space_char);
        if trimmed.len() < 3
            || !trimmed.starts_with('[')
            || !trimmed.ends_with(']')
        {
            return None;
        }
        let name = &trimmed[1..trimmed.len() - 1];
        let name = name.trim_matches(is_space_char);
        (!name.is_empty() && !name.contains('[') && !name.contains(']')).then_some(name)
    }

    /// Case-insensitive last-wins lookup returning the comment-stripped
    /// value token (`Get`).
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let mut current_section = "";
        let mut found = None;
        for line in &self.lines {
            if let Some(name) = Self::header_name(&line.text) {
                current_section = name;
                continue;
            }
            if current_section.eq_ignore_ascii_case(section)
                && let Some(pair) = parse_pair(&line.text)
                && pair.key.eq_ignore_ascii_case(key)
            {
                found = Some(pair.value);
            }
        }
        found
    }

    /// Index of the final row matching `section/key` with forward section
    /// tracking (`Set`'s scan).
    fn find_last_pair(&self, section: &str, key: &str) -> Option<usize> {
        let mut current_section = "";
        let mut found = None;
        for (index, line) in self.lines.iter().enumerate() {
            if let Some(name) = Self::header_name(&line.text) {
                current_section = name;
                continue;
            }
            if current_section.eq_ignore_ascii_case(section)
                && let Some(pair) = parse_pair(&line.text)
                && pair.key.eq_ignore_ascii_case(key)
            {
                found = Some(index);
            }
        }
        found
    }

    /// Rewrites the final occurrence of `section/key` in place, preserving
    /// the key text, spacing, and anything after the value token (inline
    /// comments). A missing pair is appended inside the matching section
    /// group, or under a fresh header at the end of the document (`Set`).
    pub fn set_value(&mut self, section: &str, key: &str, value: &str) {
        if let Some(index) = self.find_last_pair(section, key) {
            if let Some(pair) = parse_pair(&self.lines[index].text.clone()) {
                let line = &mut self.lines[index];
                line.text.replace_range(pair.value_span, value);
            }
            return;
        }
        self.append_pair(section, key, value);
    }

    /// Appends `key=value` under the last group of `section`, or under a
    /// fresh header at the end of the document, terminating any previously
    /// unterminated final line first (`Set`'s append path).
    fn append_pair(&mut self, section: &str, key: &str, value: &str) {
        let mut header = None;
        for (index, line) in self.lines.iter().enumerate() {
            if Self::header_name(&line.text).is_some_and(|name| name.eq_ignore_ascii_case(section))
            {
                header = Some(index);
            }
        }
        let insertion = match header {
            Some(header) => (header + 1..self.lines.len())
                .find(|&index| Self::header_name(&self.lines[index].text).is_some())
                .unwrap_or(self.lines.len()),
            None => self.lines.len(),
        };
        if header.is_some() {
            if insertion > 0 && self.lines[insertion - 1].terminator.is_empty() {
                self.lines[insertion - 1].terminator = self.newline;
            }
            self.lines.insert(
                insertion,
                DocLine {
                    text: format!("{key}={value}"),
                    terminator: self.newline,
                },
            );
            return;
        }
        if !self.lines.is_empty() {
            let last = self.lines.len() - 1;
            if self.lines[last].terminator.is_empty() {
                self.lines[last].terminator = self.newline;
            }
            if !self.lines[last].text.is_empty() {
                self.lines.push(DocLine {
                    text: String::new(),
                    terminator: self.newline,
                });
            }
        }
        self.lines.push(DocLine {
            text: format!("[{section}]"),
            terminator: self.newline,
        });
        self.lines.push(DocLine {
            text: format!("{key}={value}"),
            terminator: self.newline,
        });
    }

    /// Removes every row matching `section/key`, leaving headers and
    /// surrounding formatting untouched (`Erase`).
    pub fn erase(&mut self, section: &str, key: &str) {
        let mut current_section = String::new();
        let mut kept = Vec::with_capacity(self.lines.len());
        for line in self.lines.drain(..) {
            if let Some(name) = Self::header_name(&line.text) {
                current_section = name.to_string();
                kept.push(line);
                continue;
            }
            let matches = current_section.eq_ignore_ascii_case(section)
                && parse_pair(&line.text).is_some_and(|pair| pair.key.eq_ignore_ascii_case(key));
            if !matches {
                kept.push(line);
            }
        }
        self.lines = kept;
    }

    /// Re-encodes the document in its original encoding (BOM included for
    /// the BOM variants) with per-line terminators intact (`Encode`).
    pub fn render(&self) -> Vec<u8> {
        let mut text = String::new();
        for line in &self.lines {
            text.push_str(&line.text);
            text.push_str(line.terminator);
        }
        match self.encoding {
            IniEncoding::Utf8 => text.into_bytes(),
            IniEncoding::Utf8Bom => {
                let mut out = Vec::with_capacity(text.len() + 3);
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
                out.extend_from_slice(text.as_bytes());
                out
            }
            IniEncoding::Utf16Le => encode_utf16(&text, true),
            IniEncoding::Utf16Be => encode_utf16(&text, false),
        }
    }
}

fn decode_utf16(bytes: &[u8], little: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 body ends between units".to_string());
    }
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let unit = if little {
            u16::from_le_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_be_bytes([chunk[0], chunk[1]])
        };
        units.push(unit);
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

/// Dominant newline style: CRLF only when strictly more common than LF
/// (`Decode`'s `newline` rule).
fn dominant_terminator(lines: &[DocLine]) -> &'static str {
    let crlf = lines.iter().filter(|line| line.terminator == "\r\n").count();
    let lf = lines.iter().filter(|line| line.terminator == "\n").count();
    if crlf > lf { "\r\n" } else { "\n" }
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

    #[test]
    fn lookups_strip_inline_comments_and_are_last_wins() {
        let document = LauncherDoc::decode(
            b"; note\n[engine.engine]\nKeepMe=Untouched ; trailing\n\
              [Engine.GameEngine]\nFrameRateLimit=30\nFrameRateLimit = 60 ; final\n",
        )
        .expect("decodable");
        assert_eq!(
            document.get("ENGINE.ENGINE", "KeepMe"),
            Some("Untouched")
        );
        assert_eq!(document.get("Engine.GameEngine", "FrameRateLimit"), Some("60"));
        assert_eq!(document.get("Engine.GameEngine", "Missing"), None);
        assert_eq!(LauncherDoc::header_name("[Engine.GameEngine "), None);
    }

    #[test]
    fn set_value_preserves_surrounding_bytes_exactly() {
        let original =
            b"a=1\r\n[Section]\r\nKey=old ; keep me\r\nmalformed line\r\n[Next]\r\nX=Y";
        let mut document = LauncherDoc::decode(original).expect("decodable");
        document.set_value("SECTION", "key", "new");
        assert_eq!(
            String::from_utf8(document.render()).expect("utf-8"),
            "a=1\r\n[Section]\r\nKey=new ; keep me\r\nmalformed line\r\n[Next]\r\nX=Y"
        );
    }

    #[test]
    fn append_under_fresh_header_uses_dominant_newline() {
        let mut document = LauncherDoc::decode(b"[Other]\nA=B").expect("decodable");
        document.set_value("Fresh", "K", "V");
        assert_eq!(
            String::from_utf8(document.render()).expect("utf-8"),
            "[Other]\nA=B\n\n[Fresh]\nK=V\n"
        );
    }

    #[test]
    fn utf16_encodings_round_trip() {
        let text = "; comment\n[K]\nV=1\n";
        for (encoding, little) in [(IniEncoding::Utf16Le, true), (IniEncoding::Utf16Be, false)] {
            let bytes = encode_utf16(text, little);
            let document = LauncherDoc::decode(&bytes).expect("decodable");
            assert_eq!(document.encoding, encoding);
            assert_eq!(document.get("K", "V"), Some("1"));
            assert_eq!(document.render(), bytes);
        }
        assert!(LauncherDoc::decode(&[0xFF, 0xFE, 0x00]).is_err());
    }
}
