//! Golden-corpus conformance suite.
//!
//! Every case is driven programmatically from
//! `Tests/Fixtures/save-format-golden/MANIFEST.json` — the manifest is the
//! executable specification, so nothing here hardcodes file lists or
//! expected codes.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::repair::parse_save_slot;
use super::{SaveError, read_save, repair_save, write_save};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("manifest dir sits two levels below the repo root")
        .join("Tests/Fixtures/save-format-golden")
}

#[derive(Debug)]
struct Case {
    file: String,
    kind: String,
    invariant: String,
    reason_code: String,
    message_contains: String,
    sha256: String,
    size: i64,
    base: Option<String>,
}

impl Case {
    fn from_json(json: &Json) -> Case {
        let text = |key: &str| {
            json.get(key)
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Case {
            file: text("file"),
            kind: text("kind"),
            invariant: text("invariant"),
            reason_code: text("reason_code"),
            message_contains: text("message_contains"),
            sha256: text("sha256"),
            size: json.get("size").and_then(Json::as_int).unwrap_or(0),
            base: json.get("base").and_then(Json::as_str).map(str::to_owned),
        }
    }
}

fn load_cases() -> Vec<Case> {
    let path = fixture_dir().join("MANIFEST.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let root = Json::parse(&text).expect("MANIFEST.json must parse");
    root.get("cases")
        .and_then(Json::as_arr)
        .unwrap_or_else(|| panic!("MANIFEST.json has no cases array"))
        .iter()
        .map(Case::from_json)
        .collect()
}

fn read_fixture(name: &str) -> Vec<u8> {
    fs::read(fixture_dir().join(name))
        .unwrap_or_else(|error| panic!("cannot read fixture {name}: {error}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn labeled(error: &SaveError) -> &super::SaveFault {
    match error {
        SaveError::Fault(fault) => fault,
        other => panic!("expected a manifest-labeled fault, got: {other}"),
    }
}

/// Every golden save must decode and re-serialize to its exact committed
/// bytes (`save.package79.golden_parses`).
#[test]
fn golden_saves_round_trip_byte_identically() {
    let mut swept = 0usize;
    for case in load_cases() {
        if case.kind != "golden" {
            continue;
        }
        swept += 1;
        let bytes = read_fixture(&case.file);
        assert_eq!(bytes.len() as i64, case.size, "fixture {}", case.file);
        assert_eq!(sha256_hex(&bytes), case.sha256, "fixture {}", case.file);

        let archive = read_save(&bytes).unwrap_or_else(|error| panic!("{}: {error}", case.file));
        assert!(!archive.exports.is_empty(), "{} has exports", case.file);

        let mut rewritten = Vec::with_capacity(bytes.len());
        write_save(&archive, &mut rewritten)
            .unwrap_or_else(|error| panic!("{}: {error}", case.file));
        assert_eq!(rewritten, bytes, "{} must round-trip exactly", case.file);
        assert_eq!(sha256_hex(&rewritten), case.sha256, "case {}", case.file);
    }
    assert!(swept > 0, "manifest declares no golden cases");
}

/// Every corrupt mutant must be rejected with exactly its manifest
/// reason code, invariant label, and message fragment.
#[test]
fn corrupt_variants_rejected_with_manifest_reasons() {
    let mut swept = 0usize;
    for case in load_cases() {
        if case.kind != "corrupt" {
            continue;
        }
        swept += 1;
        let bytes = read_fixture(&case.file);
        assert_eq!(sha256_hex(&bytes), case.sha256, "fixture {}", case.file);
        let error = read_save(&bytes)
            .map(|_| ())
            .expect_err(format!("{} must be rejected", case.file).as_str());
        let fault = labeled(&error);
        assert_eq!(
            fault.reason_code, case.reason_code,
            "{} reason code",
            case.file
        );
        assert_eq!(fault.invariant, case.invariant, "{} invariant", case.file);
        let message = error.to_string();
        assert!(
            message.contains(&case.message_contains),
            "invariant {} violated with wrong diagnostic: {message}",
            case.invariant
        );
    }
    assert!(swept > 0, "manifest declares no corrupt cases");
}

/// The bit-flip probe parses fine — package-79 has no checksum coverage —
/// but the flipped payload bytes are observably different from the pristine
/// base save's payload.
#[test]
fn probe_bit_flip_is_structurally_undetectable_but_content_visible() {
    let mut swept = 0usize;
    for case in load_cases() {
        if case.kind != "probe" {
            continue;
        }
        swept += 1;
        let bytes = read_fixture(&case.file);
        assert_eq!(sha256_hex(&bytes), case.sha256, "fixture {}", case.file);
        let flipped = read_save(&bytes).expect("probe still parses structurally");

        let base = read_fixture(case.base.as_deref().expect("probe case records its base"));
        let pristine = read_save(&base).expect("base save parses");

        let content_changed = (0..flipped.exports.len()).any(|index| {
            let a = flipped.export_payload(index).unwrap_or(&[]);
            let b = pristine.export_payload(index).unwrap_or(&[]);
            Sha256::digest(a) != Sha256::digest(b)
        });
        assert!(
            content_changed,
            "invariant save.package79.payload.flip_changes_content"
        );
    }
    assert!(swept > 0, "manifest declares no probe cases");
}

/// The manifest must cover every fixture on disk, so future corpus additions
/// cannot silently escape this suite.
#[test]
fn manifest_covers_every_fixture_on_disk() {
    let listed: std::collections::BTreeSet<String> =
        load_cases().iter().map(|case| case.file.clone()).collect();
    let entries = fs::read_dir(fixture_dir()).expect("golden fixture directory exists");
    let mut on_disk = std::collections::BTreeSet::new();
    for entry in entries {
        let path = entry.expect("readable fixture entry").path();
        if path.extension().is_some_and(|ext| ext == "usa") {
            on_disk.insert(
                path.file_name()
                    .expect("named")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    assert_eq!(
        listed, on_disk,
        "manifest cases must cover exactly the committed fixtures",
    );
}

/// Native repair of a canonical slot: loads through the reader, re-saves,
/// leaves a backup, and swaps atomically without changing valid bytes.
#[test]
fn repair_save_rewrites_canonical_slot_with_backup() {
    let scratch = scratch_dir("repair-slot");
    fs::create_dir_all(&scratch).expect("scratch dir");
    let target = scratch.join("Save3.usa");
    let original = read_fixture("Save0.usa");
    fs::write(&target, &original).expect("seed target");

    let report = repair_save(&target).expect("repair succeeds");
    assert_eq!(report.slot, 3);
    assert_eq!(report.target, target);
    assert!(!report.rewritten, "valid golden bytes survive unchanged");
    assert!(report.backup.is_file(), "backup exists beside target");
    assert_eq!(
        fs::read(&report.backup).expect("backup readable"),
        original,
        "backup preserves the original bytes"
    );
    assert_eq!(
        fs::read(&target).expect("target readable"),
        original,
        "repaired target stays byte-identical for a valid save"
    );

    drop(fs::remove_dir_all(&scratch));
}

/// The slot-naming contract rejects non-canonical names before any I/O.
#[test]
fn repair_save_rejects_non_slot_names() {
    let scratch = scratch_dir("slot-contract");
    fs::create_dir_all(&scratch).expect("scratch dir");
    for name in ["checkpoint.usa", "Save01.usa", "SaveX.usa"] {
        let target = scratch.join(name);
        let error = repair_save(&target).expect_err("non-slot name refused");
        assert!(matches!(error, super::RepairError::Contract(_)), "{name}");
    }
    drop(fs::remove_dir_all(&scratch));
}

/// Unit coverage of the slot regex equivalent.
#[test]
fn slot_parser_matches_repair_save_regex() {
    assert_eq!(parse_save_slot(std::ffi::OsStr::new("Save0.usa")), Some(0));
    assert_eq!(
        parse_save_slot(std::ffi::OsStr::new("Save12.usa")),
        Some(12)
    );
    assert_eq!(
        parse_save_slot(std::ffi::OsStr::new("checkpoint.usa")),
        None
    );
    assert_eq!(parse_save_slot(std::ffi::OsStr::new("Save01.usa")), None);
    assert_eq!(parse_save_slot(std::ffi::OsStr::new("SaveX.usa")), None);
    assert_eq!(parse_save_slot(std::ffi::OsStr::new("Save.usa")), None);
    assert_eq!(parse_save_slot(std::ffi::OsStr::new("Save0.sav")), None);
}

fn scratch_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "hp-engine-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

// -- minimal JSON reader ----------------------------------------------------
//
// MANIFEST.json is plain UTF-8 JSON over null / bool / int / string / array /
// object. No serde dependency is permitted for this slice, so this parser
// covers exactly that domain and fails loudly on anything else.

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(text) => Some(text),
            _ => None,
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self {
            Json::Int(value) => Some(*value),
            _ => None,
        }
    }

    fn parse(text: &str) -> Result<Json, String> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        parser.skip_ws();
        let value = parser.value()?;
        parser.skip_ws();
        if parser.pos != parser.bytes.len() {
            return Err(format!("trailing input at byte {}", parser.pos));
        }
        Ok(value)
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected {:?} at byte {}, found {:?}",
                byte as char,
                self.pos,
                self.peek().map(char::from)
            ))
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(format!("expected {word:?} at byte {}", self.pos))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true").map(|()| Json::Bool(true)),
            Some(b'f') => self.literal("false").map(|()| Json::Bool(false)),
            Some(b'n') => self.literal("null").map(|()| Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            other => Err(format!(
                "unexpected {:?} at byte {}",
                other.map(char::from),
                self.pos
            )),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = self.value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(pairs));
                }
                other => {
                    return Err(format!(
                        "expected ',' or '}}' at byte {}, found {:?}",
                        self.pos,
                        other.map(char::from)
                    ));
                }
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                other => {
                    return Err(format!(
                        "expected ',' or ']' at byte {}, found {:?}",
                        self.pos,
                        other.map(char::from)
                    ));
                }
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        let digits = &self.bytes[start..self.pos];
        if digits.is_empty() || digits == b"-" {
            return Err(format!("malformed number at byte {start}"));
        }
        std::str::from_utf8(digits)
            .expect("digits are ascii")
            .parse::<i64>()
            .map(Json::Int)
            .map_err(|error| format!("number out of range at byte {start}: {error}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".to_owned()),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let escape = self
                        .peek()
                        .ok_or_else(|| "unterminated escape".to_owned())?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()?),
                        other => {
                            return Err(format!("unknown escape \\{}", char::from(other)));
                        }
                    }
                }
                Some(_) => {
                    // Consume one UTF-8 code point; input came from &str so
                    // it is valid by construction.
                    let rest = std::str::from_utf8(&self.bytes[self.pos..])
                        .expect("source was a valid str");
                    let ch = rest.chars().next().expect("non-empty remainder");
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, String> {
        let high = self.hex4()?;
        let code = if (0xD800..0xDC00).contains(&high) {
            // High surrogate: require the low surrogate half.
            if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
                return Err("lone high surrogate in \\u escape".to_owned());
            }
            self.pos += 2;
            let low = self.hex4()?;
            if !(0xDC00..0xE000).contains(&low) {
                return Err("invalid low surrogate in \\u escape".to_owned());
            }
            0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
        } else if (0xDC00..0xE000).contains(&high) {
            return Err("lone low surrogate in \\u escape".to_owned());
        } else {
            high
        };
        char::from_u32(code).ok_or_else(|| "\\u maps past U+10FFFF".to_owned())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let slice = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| "truncated \\u escape".to_owned())?;
        let text = std::str::from_utf8(slice).map_err(|_| "non-hex \\u escape".to_owned())?;
        let value =
            u32::from_str_radix(text, 16).map_err(|_| format!("non-hex \\u escape {text:?}"))?;
        self.pos += 4;
        Ok(value)
    }
}
