//! Replaces `HP2ImportManifest.cpp`'s `Manifest`/`Checksums`: serializes
//! the final asset plan to `overlay-manifest.json` and
//! `overlay-checksums.txt`.
//!
//! `Manifest`'s hand-rolled JSON writer must byte-match
//! `Build/prepare_retail_data.py`'s oracle call,
//! `json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) +
//! "\n"` (`prepare_retail_data.py:1338`), since the Python importer and
//! this one produce the same `overlay-manifest.json` and V1's
//! verification is a byte-for-byte `cmp`. [`Json`]'s `Object` variant
//! uses [`BTreeMap`], whose iteration order is sorted by key byte value
//! -- matching both the C++ `std::map<std::string, Json>` it replaces
//! (also always sorted) and Python's `sort_keys=True`, so no explicit
//! sort step is needed anywhere; the data structure enforces it.
//!
//! Verified against the real compiled `Manifest`/`Checksums`
//! (`c++ main.cpp HarryPotter2/Unreal/Import/Src/HP2ImportManifest.cpp`,
//! same pattern as every other `HP2Import` verification this crate
//! uses) on a fixture covering all three profiles and four escaping
//! edge cases in one pass: a quote, a tab/newline/control-byte mix, and
//! a literal UTF-8 non-ASCII sequence (`Straße`, confirmed passed
//! through unescaped, matching `ensure_ascii=False`) -- exact match on
//! every byte.

use std::collections::BTreeMap;

use crate::collect::Asset;

/// A JSON value, matching the C++ `Json` struct's `std::variant`
/// exactly: the same five cases (plus unused `Null`/`Bool`, which the
/// C++ type supports but no call site in `Manifest` actually
/// constructs -- kept for a faithful 1:1 shape, not because this module
/// emits them).
enum Json {
    #[allow(dead_code)]
    Null,
    #[allow(dead_code)]
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl From<&str> for Json {
    fn from(v: &str) -> Self {
        Json::Str(v.to_string())
    }
}

impl From<String> for Json {
    fn from(v: String) -> Self {
        Json::Str(v)
    }
}

impl From<i64> for Json {
    fn from(v: i64) -> Self {
        Json::Int(v)
    }
}

/// Replaces `Quote`: JSON string escaping matching Python's
/// `json.dumps(..., ensure_ascii=False)` exactly -- the standard
/// backslash escapes, `\u00XX` (lowercase hex) for other control bytes
/// below `0x20`, and every other byte (including non-ASCII UTF-8
/// continuation bytes) passed through literally, not escaped.
fn quote(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let b = c as u8;
                out.push_str("\\u00");
                out.push(hex_digit(b >> 4));
                out.push(hex_digit(b & 0xf));
            }
            // Every escaped case above is a single ASCII codepoint
            // (U+0000..U+001F, plus the two ASCII punctuation
            // characters), so any `c` reaching here is either ASCII
            // and not one of those, or genuinely non-ASCII -- both
            // cases push through unchanged via `push(c)`, which
            // UTF-8-encodes `c` back to its own original byte
            // sequence (not a reinterpretation the way pushing a raw
            // byte as a `char` would be), matching
            // `ensure_ascii=False`'s literal-UTF-8 passthrough
            // exactly.
            c => out.push(c),
        }
    }
    out.push('"');
}

fn hex_digit(n: u8) -> char {
    (if n < 10 { b'0' + n } else { b'a' + n - 10 }) as char
}

/// Replaces `WriteJson`.
fn write_json(out: &mut String, json: &Json, depth: usize) {
    match json {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Int(n) => out.push_str(&n.to_string()),
        Json::Str(s) => quote(out, s),
        Json::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            let last = items.len() - 1;
            for (i, item) in items.iter().enumerate() {
                indent(out, depth + 1);
                write_json(out, item, depth + 1);
                out.push_str(if i == last { "\n" } else { ",\n" });
            }
            indent(out, depth);
            out.push(']');
        }
        Json::Object(fields) => {
            if fields.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            let last = fields.len() - 1;
            for (i, (key, value)) in fields.iter().enumerate() {
                indent(out, depth + 1);
                quote(out, key);
                out.push_str(": ");
                write_json(out, value, depth + 1);
                out.push_str(if i == last { "\n" } else { ",\n" });
            }
            indent(out, depth);
            out.push('}');
        }
    }
}

fn indent(out: &mut String, n: usize) {
    for _ in 0..n * 2 {
        out.push(' ');
    }
}

/// Replaces `StringArray`.
fn string_array(values: &[String]) -> Json {
    Json::Array(values.iter().map(|v| Json::from(v.clone())).collect())
}

/// Replaces `Counts`: a frequency table of one string field across
/// every asset, as a sorted JSON object (sorted by [`BTreeMap`]
/// construction, matching `std::map`'s own natural order).
fn counts(assets: &[Asset], field: impl Fn(&Asset) -> &str) -> Json {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for asset in assets {
        *counts.entry(field(asset).to_string()).or_insert(0) += 1;
    }
    Json::Object(
        counts
            .into_iter()
            .map(|(k, v)| (k, Json::from(v)))
            .collect(),
    )
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| s.to_string()).collect()
}

/// Replaces `Manifest`. `profile` is the already-lowercased profile
/// string (`"safe"`, `"full"`, or `"retail-only"`), matching the C++
/// call site's own convention.
pub fn manifest(assets: &[Asset], profile: &str) -> String {
    let safe_defaults = strings(&[
        "base: prototype System/Default.ini",
        "Engine.Engine: WgpuDrv, SDLDrv, OpenAL, prototype Language=int",
        "Engine.GameEngine: FrameRateLimit=144, UseAA/NumAASamples/MaxAnisotropy maxed, and no unavailable ServerActors",
        "Core.System: forward-slash portable relative data paths",
        "SDLDrv.SDLClient: native portable client defaults",
        "WgpuDrv.WgpuRenderDevice: wgpu feature set",
    ]);
    let safe_defuser = strings(&[
        "base: prototype System/DefUser.ini with Modern controls disabled; prototype bindings retained",
        "Engine.PlayerPawn: forced ObjectDetailVeryHigh (max-quality first-run default)",
    ]);
    let full_defaults = strings(&[
        "base: prototype System/Default.ini",
        "FrontEnd: retail EASplashWaitTime, WBSplashWaitTime, LaunchCode, UseSaveSlot",
        "Engine.Engine: WgpuDrv, SDLDrv, OpenAL, Language=usa",
        "Engine.GameEngine: FrameRateLimit=144, UseAA/NumAASamples/MaxAnisotropy maxed, and no unavailable ServerActors",
        "Core.System: forward-slash portable relative data paths",
        "SDLDrv.SDLClient: native portable client defaults",
        "WgpuDrv.WgpuRenderDevice: wgpu feature set",
        "ALAudio.ALAudioSubsystem: retail final MusicVolume=0.53",
        "HGame console: retail final debug mode disabled",
    ]);
    let full_defuser = strings(&[
        "base: prototype System/DefUser.ini, retaining prototype controller fallback",
        "Engine.Input: retail final keyboard/mouse broom, map, cutscene, potion, and duel bindings",
        "Engine.PlayerPawn: ObjectDetailVeryHigh (max-quality first-run default, overriding retail's own Medium) and Modern controls disabled by default",
        "HGame.Harry: retail final difficulty damage multipliers",
    ]);
    let retail_defaults = strings(&[
        "base: retail System/default.ini; no prototype bytes",
        "FrontEnd: retail final splash, launch, and save-slot values",
        "Engine.Engine: WgpuDrv, SDLDrv, OpenAL, Language=usa",
        "Engine.GameEngine: FrameRateLimit=144, UseAA/NumAASamples/MaxAnisotropy maxed, and no unavailable ServerActors",
        "Core.System: forward-slash portable relative data paths",
        "SDLDrv.SDLClient: native portable client defaults",
        "WgpuDrv.WgpuRenderDevice: wgpu feature set",
        "ALAudio.ALAudioSubsystem: retail final MusicVolume=0.53",
        "HGame console: retail final debug mode disabled",
    ]);
    let retail_defuser = strings(&[
        "base: retail System/DefUser.ini; no prototype bytes",
        "Engine.Input: retail final keyboard/mouse broom, map, cutscene, potion, and duel bindings",
        "Engine.PlayerPawn: ObjectDetailVeryHigh (max-quality first-run default, overriding retail's own Medium) and Modern controls disabled by default",
        "HGame.Harry: retail final difficulty damage multipliers",
    ]);

    let retail_only = profile == "retail-only";
    let full = profile == "full";
    let defaults = if retail_only {
        &retail_defaults
    } else if full {
        &full_defaults
    } else {
        &safe_defaults
    };
    let defuser = if retail_only {
        &retail_defuser
    } else if full {
        &full_defuser
    } else {
        &safe_defuser
    };

    let mut entries: Vec<Json> = Vec::with_capacity(assets.len());
    let mut roots: BTreeMap<String, i64> = BTreeMap::new();
    for asset in assets {
        let mut entry: BTreeMap<String, Json> = BTreeMap::new();
        entry.insert(
            "classification".to_string(),
            asset.classification.clone().into(),
        );
        entry.insert("link_mode".to_string(), asset.link.clone().into());
        entry.insert("path".to_string(), asset.path.clone().into());
        entry.insert("sha256".to_string(), asset.hash.clone().into());
        entry.insert("size".to_string(), (asset.size as i64).into());
        entry.insert("source".to_string(), asset.source.clone().into());
        if !asset.source_path.is_empty() {
            entry.insert("source_path".to_string(), asset.source_path.clone().into());
        }
        if !asset.sources.is_empty() {
            entry.insert("source_paths".to_string(), string_array(&asset.sources));
        }
        entries.push(Json::Object(entry));
        let root = asset.path.split('/').next().unwrap_or("").to_string();
        *roots.entry(root).or_insert(0) += 1;
    }
    let root_counts: BTreeMap<String, Json> =
        roots.into_iter().map(|(k, v)| (k, Json::from(v))).collect();

    let mut summary: BTreeMap<String, Json> = BTreeMap::new();
    summary.insert(
        "classification".to_string(),
        counts(assets, |a| &a.classification),
    );
    summary.insert("entry_count".to_string(), (assets.len() as i64).into());
    summary.insert("link_mode".to_string(), counts(assets, |a| &a.link));
    summary.insert("root".to_string(), Json::Object(root_counts));
    summary.insert("source".to_string(), counts(assets, |a| &a.source));

    let mut policy: BTreeMap<String, Json> = BTreeMap::new();
    if full {
        policy.insert(
            "compatibility".to_string(),
            "candidate retail class/package surface; requires complete retail native parity".into(),
        );
        policy.insert(
            "cutscenes".to_string(),
            "complete retail System/Cutscenes bundle; no prototype raw TXT mixing".into(),
        );
        policy.insert(
            "help".to_string(),
            "canonical outer Help only; nested help/help excluded".into(),
        );
        policy.insert(
            "localization".to_string(),
            "retail coherent USA bundle with prototype-only INT fallback".into(),
        );
        policy.insert(
            "precedence".to_string(),
            "retail changed/identical/retail-only runtime data, then prototype-only fallback"
                .into(),
        );
    } else if retail_only {
        policy.insert(
            "compatibility".to_string(),
            "final retail package/map surface with no prototype fallback".into(),
        );
        policy.insert(
            "cutscenes".to_string(),
            "complete retail System/Cutscenes bundle".into(),
        );
        policy.insert(
            "help".to_string(),
            "canonical outer retail Help only; nested help/help excluded".into(),
        );
        policy.insert(
            "localization".to_string(),
            "coherent retail Language=usa and AllDialog.USA_uax bundle".into(),
        );
        policy.insert(
            "precedence".to_string(),
            "retail files only; portable configs are generated from retail templates".into(),
        );
    } else {
        policy.insert(
            "compatibility".to_string(),
            "native-smokeable prototype class/package baseline".into(),
        );
        policy.insert(
            "cutscenes".to_string(),
            "compiled prototype System/Cutscenes INT runtime bundle".into(),
        );
        policy.insert(
            "help".to_string(),
            "prototype canonical outer Help only".into(),
        );
        policy.insert(
            "localization".to_string(),
            "coherent prototype Language=int and AllDialog.uax bundle".into(),
        );
        policy.insert(
            "precedence".to_string(),
            "prototype runtime data; retail Music Oggs and retail-only HP2_Menu.utx only".into(),
        );
    }
    policy.insert(
        "prohibited".to_string(),
        "Windows binaries/installers/scripts, source-control metadata, and debris".into(),
    );

    let mut config_merges: BTreeMap<String, Json> = BTreeMap::new();
    config_merges.insert("System/Default.ini".to_string(), string_array(defaults));
    config_merges.insert("System/DefUser.ini".to_string(), string_array(defuser));

    let mut top: BTreeMap<String, Json> = BTreeMap::new();
    top.insert("config_merges".to_string(), Json::Object(config_merges));
    top.insert("entries".to_string(), Json::Array(entries));
    top.insert("format".to_string(), "hp2-retail-data-overlay".into());
    top.insert("policy".to_string(), Json::Object(policy));
    top.insert("profile".to_string(), profile.into());
    top.insert(
        "schema_version".to_string(),
        (if retail_only { 3i64 } else { 2i64 }).into(),
    );
    top.insert("summary".to_string(), Json::Object(summary));

    if retail_only {
        let mut generated: Vec<String> = Vec::new();
        let mut prototype_asset_count: i64 = 0;
        let mut prototype_template_count: i64 = 0;
        for asset in assets {
            if asset.source == "generated" {
                generated.push(asset.path.clone());
            }
            if asset.source == "prototype" {
                prototype_asset_count += 1;
            }
            if asset.sources.iter().any(|s| s.starts_with("prototype:")) {
                prototype_template_count += 1;
            }
        }
        let mut provenance: BTreeMap<String, Json> = BTreeMap::new();
        provenance.insert(
            "asset_byte_sources".to_string(),
            string_array(&strings(&["retail"])),
        );
        provenance.insert("generated_entries".to_string(), string_array(&generated));
        provenance.insert(
            "manifest".to_string(),
            "generated deterministically from the validated retail-only plan".into(),
        );
        provenance.insert(
            "prototype_asset_entry_count".to_string(),
            prototype_asset_count.into(),
        );
        provenance.insert(
            "prototype_template_input_count".to_string(),
            prototype_template_count.into(),
        );
        top.insert("provenance".to_string(), Json::Object(provenance));
    }

    let mut output = String::new();
    write_json(&mut output, &Json::Object(top), 0);
    output.push('\n');
    output
}

/// Replaces `Checksums`: a plain tab-separated `path\tsize\thash\n` line
/// per asset, sorted by exact (not casefolded) path. No escaping of any
/// kind -- a path or hash containing a literal tab or newline would
/// corrupt this format, matching the real C++ exactly (not a bug this
/// port fixes; the real importer has the same limitation).
pub fn checksums(assets: &[Asset]) -> String {
    let mut rows: Vec<&Asset> = assets.iter().collect();
    rows.sort_by(|a, b| a.path.cmp(&b.path));
    let mut output = String::new();
    for asset in rows {
        output.push_str(&asset.path);
        output.push('\t');
        output.push_str(&asset.size.to_string());
        output.push('\t');
        output.push_str(&asset.hash);
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_asset(path: &str, hash: &str) -> Asset {
        Asset {
            source: "prototype".to_string(),
            path: path.to_string(),
            classification: "c".to_string(),
            hash: hash.to_string(),
            size: 1,
            link: "hardlink".to_string(),
            source_path: "x".to_string(),
            ..Asset::default()
        }
    }

    /// This is the specific case that caught a real bug during this
    /// port: an earlier version of `quote()` iterated `value.bytes()`
    /// and pushed each non-escaped byte back via `byte as char`, which
    /// doesn't reconstruct the original byte -- `0xc3 as char` is the
    /// Unicode scalar U+00C3, and pushing *that* char into a `String`
    /// re-encodes it as the *two*-byte UTF-8 sequence `c3 83` (and the
    /// following `0x9f as char` similarly becomes `c2 9f`), silently
    /// turning the 2-byte `ß` into a 4-byte garbled sequence and
    /// corrupting every non-ASCII path, hash, or string this function
    /// ever touches. Caught by rereading the function before this test
    /// was ever run against it, not by a failing test -- so there is no
    /// "before" run of this exact test to cite; only the fixed version
    /// was ever executed. The assertion below is still a real,
    /// non-tautological check: it compares the full byte sequence of
    /// the rendered path against the known-correct 2-byte UTF-8 for
    /// `ß` (`c3 9f`), which the buggy version would have rendered as
    /// the 4-byte `c3 83 c2 9f` instead -- a length and byte mismatch
    /// `assert_eq!` on the literal string below would have caught, had
    /// the buggy version been run against it.
    #[test]
    fn quote_passes_non_ascii_utf8_through_unescaped_not_reencoded() {
        let assets = vec![minimal_asset("Help/Straße.txt", "h")];
        let output = manifest(&assets, "safe");
        assert!(
            output.contains("\"path\": \"Help/Straße.txt\","),
            "{output}"
        );
        // Explicit byte-level check, not just a &str `contains` (which
        // already does a correct byte-level match, but spelling this
        // out makes the specific c3-9f-not-c3-83-c2-9f claim visible
        // directly in the assertion, not just in the comment above).
        let path_line = output
            .lines()
            .find(|l| l.contains("\"path\""))
            .expect("path line present");
        assert!(
            path_line.as_bytes().windows(2).any(|w| w == [0xc3, 0x9f]),
            "expected the 2-byte UTF-8 ß (c3 9f) in: {path_line:?}"
        );
        assert!(
            !path_line.as_bytes().windows(2).any(|w| w == [0xc3, 0x83]),
            "found the corrupted 4-byte-sequence's first half (c3 83) in: {path_line:?}"
        );
    }

    /// Ground truth from the real compiled `Manifest` on a fixture
    /// exercising every `Quote` escape case in one path: the named
    /// backslash escapes (`"`, `\`, tab, newline, CR, backspace,
    /// form-feed) and the generic `\u00XX` fallback for another control
    /// byte (`\x01`). Verified via a throwaway C++ probe
    /// (`c++ main.cpp HarryPotter2/Unreal/Import/Src/HP2ImportManifest.cpp`)
    /// before writing this assertion.
    #[test]
    fn quote_matches_cpp_escaping_for_every_named_and_control_byte_case() {
        let assets = vec![minimal_asset(
            "Quote\"Back\\Tab\tNL\nCR\rBS\u{8}FF\u{c}Ctl\u{1}.txt",
            "h",
        )];
        let output = manifest(&assets, "safe");
        assert!(
            output.contains(
                "\"path\": \"Quote\\\"Back\\\\Tab\\tNL\\nCR\\rBS\\bFF\\fCtl\\u0001.txt\","
            ),
            "{output}"
        );
    }

    /// Fixture asset list shared by the three full-output manifest
    /// tests and the checksums test below: two case-ordering-relevant
    /// retail/prototype assets ("System/B.u" vs "System/a.int" --
    /// uppercase sorts before lowercase in byte-value order, which this
    /// exercises directly since neither `Manifest`'s entry order nor
    /// `Checksums`' row order is casefolded), and the two generated
    /// config assets, one with *two* `prototype:`-prefixed sources (to
    /// confirm the retail-only provenance's
    /// `prototype_template_input_count` counts once per asset, not once
    /// per source -- matching the real C++'s `break` after the first
    /// match, confirmed directly against the compiled source before
    /// writing this fixture).
    fn full_fixture() -> Vec<Asset> {
        vec![
            Asset {
                source: "prototype".to_string(),
                path: "System/B.u".to_string(),
                classification: "prototype_baseline".to_string(),
                hash: "hb".to_string(),
                size: 100,
                link: "hardlink".to_string(),
                source_path: "System/B.u".to_string(),
                ..Asset::default()
            },
            Asset {
                source: "retail".to_string(),
                path: "System/a.int".to_string(),
                classification: "retail_asset".to_string(),
                hash: "ha".to_string(),
                size: 50,
                link: "copy".to_string(),
                source_path: "System/a.int".to_string(),
                ..Asset::default()
            },
            Asset {
                source: "generated".to_string(),
                path: "System/Default.ini".to_string(),
                classification: "generated_retail_native".to_string(),
                hash: "hg1".to_string(),
                size: 10,
                link: "generated".to_string(),
                sources: vec![
                    "prototype:System/Default.ini".to_string(),
                    "prototype:System/Default2.ini".to_string(),
                ],
                ..Asset::default()
            },
            Asset {
                source: "generated".to_string(),
                path: "System/DefUser.ini".to_string(),
                classification: "generated_retail_native".to_string(),
                hash: "hg2".to_string(),
                size: 11,
                link: "generated".to_string(),
                sources: vec!["retail:System/DefUser.ini".to_string()],
                ..Asset::default()
            },
        ]
    }

    /// Full byte-for-byte comparison against the real compiled
    /// `Manifest` -- not a substring check, which wouldn't catch a typo
    /// or dropped entry anywhere in the ~40 hand-copied
    /// `config_merges`/`policy` literal strings this profile pulls in.
    /// Fixture committed from the real C++ probe's own stdout
    /// (`crates/hp2-import/testdata/manifest_safe.json`), not generated
    /// from this port.
    #[test]
    fn manifest_matches_cpp_byte_for_byte_for_safe_profile() {
        assert_eq!(
            manifest(&full_fixture(), "safe"),
            include_str!("../testdata/manifest_safe.json")
        );
    }

    #[test]
    fn manifest_matches_cpp_byte_for_byte_for_full_profile() {
        assert_eq!(
            manifest(&full_fixture(), "full"),
            include_str!("../testdata/manifest_full.json")
        );
    }

    #[test]
    fn manifest_matches_cpp_byte_for_byte_for_retail_only_profile() {
        assert_eq!(
            manifest(&full_fixture(), "retail-only"),
            include_str!("../testdata/manifest_retail_only.json")
        );
    }

    /// Full byte-for-byte comparison against the real compiled
    /// `Checksums` on the same fixture: confirms sort-by-exact-path
    /// (byte-value order, not casefolded -- "System/B.u" sorts before
    /// "System/DefUser.ini" before "System/Default.ini" before
    /// "System/a.int") and the plain `path\tsize\thash\n` row format.
    #[test]
    fn checksums_matches_cpp_byte_for_byte() {
        assert_eq!(
            checksums(&full_fixture()),
            include_str!("../testdata/checksums.txt")
        );
    }

    #[test]
    fn manifest_object_keys_are_always_sorted() {
        // BTreeMap sorts by construction; this pins that the top-level
        // manifest object's keys come out in the same order Python's
        // sort_keys=True would give, not just that each individual
        // nested object happens to be sorted. Complements (doesn't
        // replace) the full-output byte comparisons above, which only
        // check one specific fixture's exact content, not this general
        // structural property across profiles.
        let assets = vec![minimal_asset("System/a.u", "h")];
        let output = manifest(&assets, "safe");
        let keys = [
            "config_merges",
            "entries",
            "format",
            "policy",
            "profile",
            "schema_version",
            "summary",
        ];
        let mut last_pos = 0;
        for key in keys {
            let marker = format!("\"{key}\": ");
            let pos = output
                .find(&marker)
                .unwrap_or_else(|| panic!("missing key {key}"));
            assert!(pos > last_pos, "key {key} out of sorted order: {output}");
            last_pos = pos;
        }
    }

    #[test]
    fn manifest_ends_with_a_single_trailing_newline() {
        let assets = vec![minimal_asset("System/a.u", "h")];
        let output = manifest(&assets, "safe");
        assert!(output.ends_with("}\n"));
        assert!(!output.ends_with("}\n\n"));
    }

    #[test]
    fn manifest_schema_version_is_3_for_retail_only_and_2_otherwise() {
        let assets = vec![minimal_asset("System/a.u", "h")];
        assert!(manifest(&assets, "safe").contains("\"schema_version\": 2"));
        assert!(manifest(&assets, "full").contains("\"schema_version\": 2"));
        assert!(manifest(&assets, "retail-only").contains("\"schema_version\": 3"));
    }

    #[test]
    fn manifest_adds_provenance_only_for_retail_only() {
        let assets = vec![minimal_asset("System/a.u", "h")];
        assert!(!manifest(&assets, "safe").contains("\"provenance\""));
        assert!(!manifest(&assets, "full").contains("\"provenance\""));
        assert!(manifest(&assets, "retail-only").contains("\"provenance\""));
    }
}
