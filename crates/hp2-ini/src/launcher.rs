//! Line-by-line port of the ini codec inside
//! `HarryPotter2/Unreal/SDLLaunch/Src/HP2LauncherStore.cpp`: `Document`,
//! `Line`, `Encoding`, `Decode`/`Encode`, `Section`/`Key`, and `Get`/`Set`/
//! `Erase`. This is a distinct implementation from [`crate`]'s importer
//! codec (different struct shape, different comment/escaping rules,
//! different "last match wins" vs "ambiguous is an error" semantics) — kept
//! as its own module rather than merged with it.
//!
//! Out of scope, and left in C++: `ReadFile`, the atomic stage/backup/
//! publish/rollback machinery, `LoadDocument`/`SelectDocument`/
//! `PublishLauncherDocument`, and every settings-policy consumer
//! (`LoadBool`/`LoadSettings`/`ApplyGame`/`ApplyUser`/…) — none of that is
//! ini parsing, and the policy layer is slated for `hp2-launcher` later in
//! Phase 1. C++ never copies a `Document`, only ever holds one by value in a
//! local scope and mutates it by reference, so the FFI layer in `hp2-ffi`
//! exposes it as an opaque handle (new/decode/free/encode/get/set/erase)
//! rather than marshaling the whole structure across every call.
//!
//! Unlike the importer's `Lower` (which falls back to full Unicode
//! casefolding for non-ASCII), this file's C++ `Lower`/`Same` work one
//! `char` at a time through plain `std::tolower` — already ASCII-only. No
//! casefold deviation applies here.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16LE,
    Utf16BE,
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    ending: String,
}

/// Opaque (to C++) parsed document: one `Launcher.ini`/`Game.ini`/`User.ini`
/// file. Port of the C++ `Document` struct.
#[derive(Clone, Debug)]
pub struct Document {
    encoding: Encoding,
    newline: String,
    lines: Vec<Line>,
}

struct ParsedKey {
    key: String,
    value: String,
    value_begin: usize,
    value_end: usize,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    /// Port of `Document document;` (default member initializers:
    /// `encoding = Utf8`, `newline = "\n"`, empty `lines`).
    pub fn new() -> Self {
        Document {
            encoding: Encoding::Utf8,
            newline: "\n".to_string(),
            lines: Vec::new(),
        }
    }

    /// Port of `Decode`.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        decode(bytes)
    }

    /// Port of `Encode`.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        encode(self)
    }

    /// Port of `Get`. `None` matches `Get` returning `false`.
    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        get(self, section, key)
    }

    /// Port of `Set`.
    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        set(self, section, key, value)
    }

    /// Port of `Erase`.
    pub fn erase(&mut self, section: &str, key: &str) {
        erase(self, section, key)
    }
}

fn space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | 0x0b | 0x0c | b'\r' | b'\n')
}

fn trim(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut begin = 0usize;
    let mut end = bytes.len();
    while begin < end && space(bytes[begin]) {
        begin += 1;
    }
    while end > begin && space(bytes[end - 1]) {
        end -= 1;
    }
    &s[begin..end]
}

/// ASCII-only case-insensitive compare, matching the C++ `Same`/`Lower`
/// (plain `std::tolower` per char, not Unicode casefolding).
fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn get_utf8(text: &[u8], i: &mut usize) -> Option<u32> {
    let first = text[*i] as u32;
    *i += 1;
    if first < 0x80 {
        return Some(first);
    }
    let (count, mut cp, minimum): (usize, u32, u32) = if first & 0xe0 == 0xc0 {
        (1, first & 0x1f, 0x80)
    } else if first & 0xf0 == 0xe0 {
        (2, first & 0x0f, 0x800)
    } else if first & 0xf8 == 0xf0 {
        (3, first & 0x07, 0x10000)
    } else {
        return None;
    };
    if *i + count > text.len() {
        return None;
    }
    for _ in 0..count {
        let next = text[*i] as u32;
        *i += 1;
        if next & 0xc0 != 0x80 {
            return None;
        }
        cp = (cp << 6) | (next & 0x3f);
    }
    if cp >= minimum && cp <= 0x10ffff && !(0xd800..=0xdfff).contains(&cp) {
        Some(cp)
    } else {
        None
    }
}

fn decode16(bytes: &[u8], little: bool) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err("UTF-16 configuration has an incomplete code unit.".to_string());
    }
    let unit = |i: usize| -> u32 {
        let a = bytes[i] as u32;
        let b = bytes[i + 1] as u32;
        if little { a | (b << 8) } else { (a << 8) | b }
    };
    let mut text = String::new();
    let mut i = 2usize; // skips the BOM the caller already consumed.
    while i < bytes.len() {
        let mut cp = unit(i);
        if (0xd800..=0xdbff).contains(&cp) {
            if i + 3 >= bytes.len() {
                return Err("UTF-16 configuration has an incomplete surrogate.".to_string());
            }
            let low = unit(i + 2);
            if !(0xdc00..=0xdfff).contains(&low) {
                return Err("UTF-16 configuration has an invalid surrogate.".to_string());
            }
            cp = 0x10000 + ((cp - 0xd800) << 10) + low - 0xdc00;
            i += 2;
        } else if (0xdc00..=0xdfff).contains(&cp) {
            return Err("UTF-16 configuration has an unmatched surrogate.".to_string());
        }
        if let Some(c) = char::from_u32(cp) {
            text.push(c);
        }
        i += 2;
    }
    Ok(text)
}

fn encode16(text: &[u8], little: bool) -> Result<Vec<u8>, String> {
    let mut bytes = vec![
        if little { 0xff } else { 0xfe },
        if little { 0xfe } else { 0xff },
    ];
    let put = |bytes: &mut Vec<u8>, unit: u16| {
        if little {
            bytes.push((unit & 0xff) as u8);
            bytes.push((unit >> 8) as u8);
        } else {
            bytes.push((unit >> 8) as u8);
            bytes.push((unit & 0xff) as u8);
        }
    };
    let mut i = 0usize;
    while i < text.len() {
        let cp = match get_utf8(text, &mut i) {
            Some(cp) => cp,
            None => return Err("Configuration contains invalid UTF-8 text.".to_string()),
        };
        if cp <= 0xffff {
            put(&mut bytes, cp as u16);
        } else {
            let cp = cp - 0x10000;
            put(&mut bytes, (0xd800 | (cp >> 10)) as u16);
            put(&mut bytes, (0xdc00 | (cp & 0x3ff)) as u16);
        }
    }
    Ok(bytes)
}

fn decode(bytes: &[u8]) -> Result<Document, String> {
    let encoding;
    let text_bytes: Vec<u8>;
    if bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] == 0xfe {
        encoding = Encoding::Utf16LE;
        text_bytes = decode16(bytes, true)?.into_bytes();
    } else if bytes.len() >= 2 && bytes[0] == 0xfe && bytes[1] == 0xff {
        encoding = Encoding::Utf16BE;
        text_bytes = decode16(bytes, false)?.into_bytes();
    } else if bytes.len() >= 3 && bytes[0] == 0xef && bytes[1] == 0xbb && bytes[2] == 0xbf {
        encoding = Encoding::Utf8Bom;
        text_bytes = bytes[3..].to_vec();
    } else {
        encoding = Encoding::Utf8;
        text_bytes = bytes.to_vec();
    }
    let mut offset = 0usize;
    while offset < text_bytes.len() {
        if get_utf8(&text_bytes, &mut offset).is_none() {
            return Err("Configuration contains invalid UTF-8 text.".to_string());
        }
    }
    // SAFETY: the scan above confirmed every byte decodes as UTF-8.
    let text = String::from_utf8(text_bytes).expect("validated UTF-8");
    let raw = text.as_bytes();
    let mut lines = Vec::new();
    let mut begin = 0usize;
    let mut crlf = 0u32;
    let mut lf = 0u32;
    while begin < text.len() {
        match text[begin..].find('\n') {
            None => {
                lines.push(Line {
                    text: text[begin..].to_string(),
                    ending: String::new(),
                });
                break;
            }
            Some(rel) => {
                let end = begin + rel;
                if end > begin && raw[end - 1] == b'\r' {
                    lines.push(Line {
                        text: text[begin..end - 1].to_string(),
                        ending: "\r\n".to_string(),
                    });
                    crlf += 1;
                } else {
                    lines.push(Line {
                        text: text[begin..end].to_string(),
                        ending: "\n".to_string(),
                    });
                    lf += 1;
                }
                begin = end + 1;
            }
        }
    }
    let newline = if crlf > lf { "\r\n" } else { "\n" }.to_string();
    Ok(Document {
        encoding,
        newline,
        lines,
    })
}

fn encode(document: &Document) -> Result<Vec<u8>, String> {
    let mut text = String::new();
    for line in &document.lines {
        text.push_str(&line.text);
        text.push_str(&line.ending);
    }
    match document.encoding {
        Encoding::Utf8 => Ok(text.into_bytes()),
        Encoding::Utf8Bom => {
            let mut bytes = vec![0xef, 0xbb, 0xbf];
            bytes.extend_from_slice(text.as_bytes());
            Ok(bytes)
        }
        Encoding::Utf16LE => encode16(text.as_bytes(), true),
        Encoding::Utf16BE => encode16(text.as_bytes(), false),
    }
}

fn parse_section(line: &str) -> Option<String> {
    let text = trim(line);
    let bytes = text.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'[' || bytes[bytes.len() - 1] != b']' {
        return None;
    }
    let inner = trim(&text[1..text.len() - 1]);
    if !inner.is_empty() && !inner.contains('[') && !inner.contains(']') {
        Some(inner.to_string())
    } else {
        None
    }
}

fn parse_key(line: &str) -> Option<ParsedKey> {
    let bytes = line.as_bytes();
    let mut begin = 0usize;
    while begin < bytes.len() && space(bytes[begin]) {
        begin += 1;
    }
    if begin == bytes.len() || bytes[begin] == b';' || bytes[begin] == b'#' {
        return None;
    }
    let equals = line[begin..].find('=').map(|p| p + begin)?;
    let key_text = trim(&line[begin..equals]).to_string();
    if key_text.is_empty() {
        return None;
    }
    let mut value_begin = equals + 1;
    while value_begin < bytes.len() && space(bytes[value_begin]) {
        value_begin += 1;
    }
    let mut comment = bytes.len();
    let mut i = value_begin;
    while i < bytes.len() {
        if (bytes[i] == b';' || bytes[i] == b'#') && (i == value_begin || space(bytes[i - 1])) {
            comment = i;
            break;
        }
        i += 1;
    }
    let mut value_end = comment;
    while value_end > value_begin && space(bytes[value_end - 1]) {
        value_end -= 1;
    }
    Some(ParsedKey {
        key: key_text,
        value: line[value_begin..value_end].to_string(),
        value_begin,
        value_end,
    })
}

fn get(document: &Document, wanted_section: &str, wanted_key: &str) -> Option<String> {
    let mut section = String::new();
    let mut found = None;
    for line in &document.lines {
        if let Some(next) = parse_section(&line.text) {
            section = next;
            continue;
        }
        if same(&section, wanted_section)
            && let Some(parsed) = parse_key(&line.text)
            && same(&parsed.key, wanted_key)
        {
            found = Some(trim(&parsed.value).to_string());
        }
    }
    found
}

fn set(document: &mut Document, wanted_section: &str, wanted_key: &str, value: &str) {
    let newline = document.newline.clone();
    let mut section = String::new();
    let mut final_idx = document.lines.len();
    for (i, line) in document.lines.iter().enumerate() {
        if let Some(next) = parse_section(&line.text) {
            section = next;
            continue;
        }
        if same(&section, wanted_section)
            && let Some(parsed) = parse_key(&line.text)
            && same(&parsed.key, wanted_key)
        {
            final_idx = i;
        }
    }
    if final_idx != document.lines.len() {
        let parsed = parse_key(&document.lines[final_idx].text).expect("matched by the scan above");
        document.lines[final_idx]
            .text
            .replace_range(parsed.value_begin..parsed.value_end, value);
        return;
    }
    let mut header = document.lines.len();
    for (i, line) in document.lines.iter().enumerate() {
        if let Some(next) = parse_section(&line.text)
            && same(&next, wanted_section)
        {
            header = i;
        }
    }
    let mut insertion = document.lines.len();
    if header != document.lines.len() {
        for (i, line) in document.lines.iter().enumerate().skip(header + 1) {
            if parse_section(&line.text).is_some() {
                insertion = i;
                break;
            }
        }
    }
    if header != document.lines.len() {
        if insertion > 0 && document.lines[insertion - 1].ending.is_empty() {
            document.lines[insertion - 1].ending = newline.clone();
        }
        document.lines.insert(
            insertion,
            Line {
                text: format!("{wanted_key}={value}"),
                ending: newline,
            },
        );
        return;
    }
    if !document.lines.is_empty() {
        let needs_blank = !document.lines.last().unwrap().text.is_empty();
        let last = document.lines.last_mut().unwrap();
        if last.ending.is_empty() {
            last.ending = newline.clone();
        }
        if needs_blank {
            document.lines.push(Line {
                text: String::new(),
                ending: newline.clone(),
            });
        }
    }
    document.lines.push(Line {
        text: format!("[{wanted_section}]"),
        ending: newline.clone(),
    });
    document.lines.push(Line {
        text: format!("{wanted_key}={value}"),
        ending: newline,
    });
}

fn erase(document: &mut Document, wanted_section: &str, wanted_key: &str) {
    let mut section = String::new();
    let mut kept = Vec::with_capacity(document.lines.len());
    for line in document.lines.drain(..) {
        if let Some(next) = parse_section(&line.text) {
            section = next;
            kept.push(line);
            continue;
        }
        if same(&section, wanted_section)
            && let Some(parsed) = parse_key(&line.text)
            && same(&parsed.key, wanted_key)
        {
            continue;
        }
        kept.push(line);
    }
    document.lines = kept;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_tracks_majority_newline_style() {
        let doc = Document::decode(b"a=1\r\nb=2\r\nc=3\n").unwrap();
        assert_eq!(doc.newline, "\r\n");
        assert_eq!(doc.lines.len(), 3);
        assert_eq!(doc.lines[0].text, "a=1");
        assert_eq!(doc.lines[0].ending, "\r\n");
        assert_eq!(doc.lines[2].ending, "\n");
    }

    #[test]
    fn decode_last_line_without_terminator_has_empty_ending() {
        let doc = Document::decode(b"[S]\nK=1").unwrap();
        assert_eq!(doc.lines.last().unwrap().text, "K=1");
        assert_eq!(doc.lines.last().unwrap().ending, "");
    }

    #[test]
    fn get_returns_last_match_in_section() {
        let doc = Document::decode(b"[S]\nK=1\nK=2\n").unwrap();
        assert_eq!(doc.get("S", "K").as_deref(), Some("2"));
    }

    #[test]
    fn get_respects_comment_only_after_whitespace_or_at_start() {
        let doc = Document::decode(b"[S]\nK=a;b\nJ=a ;c\n").unwrap();
        // ';' immediately after a non-space char is part of the value.
        assert_eq!(doc.get("S", "K").as_deref(), Some("a;b"));
        // ';' preceded by a space starts a comment.
        assert_eq!(doc.get("S", "J").as_deref(), Some("a"));
    }

    #[test]
    fn set_replaces_in_place_preserving_the_rest_of_the_line() {
        let mut doc = Document::decode(b"[S]\nK=old ; keep this comment\n").unwrap();
        doc.set("S", "K", "new");
        assert_eq!(doc.lines[1].text, "K=new ; keep this comment");
    }

    #[test]
    fn set_inserts_at_end_of_existing_section() {
        let mut doc = Document::decode(b"[S]\nA=1\n[T]\nB=2\n").unwrap();
        doc.set("S", "C", "3");
        assert_eq!(doc.get("S", "C").as_deref(), Some("3"));
        // inserted before [T], not at the end of the document
        let idx_c = doc.lines.iter().position(|l| l.text == "C=3").unwrap();
        let idx_t = doc.lines.iter().position(|l| l.text == "[T]").unwrap();
        assert!(idx_c < idx_t);
    }

    #[test]
    fn set_creates_missing_section_with_blank_separator() {
        let mut doc = Document::decode(b"[A]\nK=1").unwrap();
        doc.set("B", "K", "2");
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["[A]", "K=1", "", "[B]", "K=2"]);
    }

    #[test]
    fn set_on_empty_document_adds_no_leading_blank() {
        let mut doc = Document::new();
        doc.set("A", "K", "1");
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["[A]", "K=1"]);
    }

    #[test]
    fn erase_removes_every_matching_row_case_insensitively() {
        let mut doc = Document::decode(b"[S]\nk=1\nK=2\nJ=3\n").unwrap();
        doc.erase("s", "k");
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, vec!["[S]", "J=3"]);
    }

    #[test]
    fn encode_round_trips_crlf_bytes() {
        let doc = Document::decode(b"[S]\r\nK=1\r\n").unwrap();
        assert_eq!(doc.encode().unwrap(), b"[S]\r\nK=1\r\n".to_vec());
    }

    #[test]
    fn decode_rejects_invalid_utf8() {
        let err = Document::decode(&[b'a', 0xff]).unwrap_err();
        assert_eq!(err, "Configuration contains invalid UTF-8 text.");
    }

    #[test]
    fn utf16_round_trip_preserves_encoding() {
        let mut doc = Document::decode(&[0xff, 0xfe, b'[', 0, b'S', 0, b']', 0]).unwrap();
        assert_eq!(doc.lines[0].text, "[S]");
        doc.set("S", "K", "v");
        let bytes = doc.encode().unwrap();
        let roundtrip = Document::decode(&bytes).unwrap();
        assert_eq!(roundtrip.get("S", "K").as_deref(), Some("v"));
        assert_eq!(&bytes[0..2], &[0xff, 0xfe]);
    }
}
