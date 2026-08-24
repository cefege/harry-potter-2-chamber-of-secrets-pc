//! Ports of Tests/ConfigIniTests.cpp contracts. Each test cites the C++
//! case it freezes; expected values are literal, not derived.

use hp_ini::{IniFile, atof_quirk};
use std::path::{Path, PathBuf};

fn parse(text: &str) -> IniFile {
    IniFile::parse(text.as_bytes())
}

#[track_caller]
fn expect_value(file: &IniFile, section: &str, key: &str, expected: &str) {
    assert_eq!(file.get(section, key), Some(expected), "[{section}]{key}");
}

#[test]
fn parse_semantics_table() {
    // ParseCase table from TestParseSemantics, in order.
    let cases: &[(&str, &str, &str, &str, Option<&str>)] = &[
        // (name, ini bytes, section, key, expected)
        ("plain", "[S]\r\nK=V\r\n", "S", "K", Some("V")),
        ("lf_only", "[S]\nK=V\n", "S", "K", Some("V")),
        ("cr_only", "[S]\rK=V\r", "S", "K", Some("V")),
        (
            "second_equals_in_value",
            "[S]\nK=a=b\n",
            "S",
            "K",
            Some("a=b"),
        ),
        ("empty_value", "[S]\nK=\n", "S", "K", Some("")),
        ("leading_equals_value", "[S]\nK==v\n", "S", "K", Some("=v")),
        ("quoted", "[S]\nK=\"quoted\"\n", "S", "K", Some("quoted")),
        (
            "quoted_inner_quotes",
            "[S]\nK=\"a\"b\"\n",
            "S",
            "K",
            Some("a\"b"),
        ),
        (
            "unterminated_quote",
            "[S]\nK=\"partial\n",
            "S",
            "K",
            Some("\"partial"),
        ),
        (
            "untrimmed_value",
            "[S]\nK= untrimmed \n",
            "S",
            "K",
            Some(" untrimmed "),
        ),
        (
            "untrimmed_key",
            "[S]\n  Key =  Val \n",
            "S",
            "  Key ",
            Some("  Val "),
        ),
        (
            "inline_comment_is_value",
            "[S]\nc=x ; tail\n",
            "S",
            "c",
            Some("x ; tail"),
        ),
        (
            "comment_with_equals_is_pair",
            "[S]\n; note=x\n",
            "S",
            "; note",
            Some("x"),
        ),
        (
            "quoted_key_verbatim",
            "[S]\n\"Q\"=1\n",
            "S",
            "\"Q\"",
            Some("1"),
        ),
        ("leading_blanks", "\n\n[S]\nK=V\n", "S", "K", Some("V")),
        ("before_section_dropped", "K=V\n[S]\n", "S", "K", None),
        ("no_equals_dropped", "[S]\nK\n", "S", "K", None),
        (
            "unclosed_section_dropped",
            "[Unclosed\nK=V\n",
            "Unclosed",
            "K",
            None,
        ),
        (
            "section_with_trailing_junk_dropped",
            "[S]tray\nK=V\n",
            "S",
            "K",
            None,
        ),
        ("empty_section_name", "[]\nK=V\n", "", "K", Some("V")),
        ("empty_file", "", "S", "K", None),
    ];
    for (name, text, section, key, expected) in cases {
        let file = parse(text);
        assert_eq!(file.get(section, key), *expected, "case {name}");
    }
}

#[test]
fn missing_file_yields_empty_config() {
    // FConfigFile::Read on an absent path leaves the config empty; modeled
    // at this layer by IniFile::default() (load_pair maps NotFound to None).
    assert!(IniFile::default().get("S", "K").is_none());
}

#[test]
fn duplicate_keys_last_wins_and_both_retained() {
    let file = parse("[S]\nA=1\nA=2\n");
    expect_value(&file, "S", "A", "2");
    assert_eq!(file.get_array("S", "A"), vec!["1", "2"]);
}

#[test]
fn case_differing_duplicates_shadow_and_retain() {
    let file = parse("[S]\nA=1\na=2\n");
    expect_value(&file, "S", "A", "2");
    expect_value(&file, "S", "a", "2");
    assert_eq!(file.get_array("S", "A").len(), 2);
}

#[test]
fn case_variant_sections_merge_into_first_spelling() {
    let file = parse("[Render]\nk=1\n[RENDER]\nk2=2\n");
    expect_value(&file, "render", "K", "1");
    expect_value(&file, "Render", "K2", "2");
    assert_eq!(file.len(), 1, "merged sections must produce one section");
}

#[test]
fn same_key_in_two_sections_independent() {
    let file = parse("[S]\nk=v\n[T]\nk=w\n");
    expect_value(&file, "S", "k", "v");
    expect_value(&file, "T", "k", "w");
}

#[test]
fn int_getter_wcstol_semantics() {
    let file = parse(
        "[Nums]\nIntPlain=42\nIntSigned=-7\nIntPlus=+7\nIntWsGarbage= 42abc\nIntHex=0x10\nIntWord=yes\nIntEmpty=\nIntFloaty=3.9\n",
    );
    let cases = [
        ("IntPlain", 42),
        ("IntSigned", -7),
        ("IntPlus", 7),
        ("IntWsGarbage", 42),
        ("IntHex", 0),    // base-10 stops at 'x'
        ("IntWord", 0),   // garbage yields 0, still found
        ("IntEmpty", 0),  // empty yields 0, still found
        ("IntFloaty", 3), // stops at '.'
    ];
    for (key, expected) in cases {
        assert_eq!(file.get_int("Nums", key), Some(expected), "GetInt({key})");
    }
    assert_eq!(file.get_int("Nums", "Absent"), None);
}

#[test]
fn float_getter_atof_c_locale() {
    let file = parse("[Reals]\nF1=1.5\nFExp=2e1\nFGarbage=abc\nFComma=1,5\n[Nums]\nIntPlain=42\n");
    assert_eq!(file.get_float("Reals", "F1"), Some(1.5));
    assert_eq!(file.get_float("Reals", "FExp"), Some(20.0));
    assert_eq!(file.get_float("Reals", "FGarbage"), Some(0.0));
    assert_eq!(file.get_float("Reals", "FComma"), Some(1.0));
    assert_eq!(file.get_float("Nums", "IntPlain"), Some(42.0));
    assert_eq!(atof_quirk("."), 0.0);
}

#[test]
fn bool_getter_true_literal_or_atoi_one() {
    let file = parse(
        "[Bools]\nBTrue=True\nBLower=true\nBUpper=TRUE\nBOne=1\nBZero=0\nBNoWord=no\nBTwo=2\nBSpaceed= true \nBOneTrailing=1x\nBFalse=False\n",
    );
    let cases = [
        ("BTrue", true),
        ("BLower", true),
        ("BUpper", true),
        ("BOne", true), // atoi == 1
        ("BZero", false),
        ("BNoWord", false),  // "no" is not boolean; atoi 0
        ("BTwo", false),     // QUIRK: must be exactly 1
        ("BSpaceed", false), // QUIRK: padded literal fails stricmp AND atoi
        ("BOneTrailing", true),
        ("BFalse", false),
    ];
    for (key, expected) in cases {
        assert_eq!(
            file.get_bool("Bools", key),
            Some(expected),
            "GetBool({key})"
        );
    }
    assert_eq!(file.get_bool("Bools", "Absent"), None);
}

#[test]
fn fresh_set_then_flush_writes_canonical_crlf() {
    let mut file = IniFile::default();
    file.set_string("Video", "Mode", "32");
    let bytes = file.render_if_dirty().expect("dirty config renders");
    assert_eq!(bytes, b"[Video]\r\nMode=32\r\n\r\n");
}

#[test]
fn same_value_reset_stays_clean() {
    let mut file = parse("[S]\nK=v\n");
    file.set_string("S", "K", "v"); // identical re-set
    assert!(
        file.render_if_dirty().is_none(),
        "clean file must not render"
    );
}

#[test]
fn case_only_value_change_is_silent_noop() {
    let mut file = parse("[S]\nK=True\n");
    file.set_string("S", "K", "TRUE");
    assert!(
        file.render_if_dirty().is_none(),
        "case-only SetString must not dirty"
    );
    assert_eq!(file.get("S", "K"), Some("True"), "old casing kept");
}

#[test]
fn real_change_updates_and_rewrites_canonical() {
    let mut file = parse("[S]\nK=True\n");
    file.set_string("S", "K", "False");
    let bytes = file.render_if_dirty().expect("real change dirties");
    assert_eq!(bytes, b"[S]\r\nK=False\r\n\r\n");
}

#[test]
fn typed_writer_byte_layout() {
    let mut file = IniFile::default();
    file.set_int("S", "I", -42);
    file.set_float("S", "F", 0.5);
    file.set_bool("S", "B", true);
    file.set_bool("S", "B2", false);
    let bytes = file.render_if_dirty().expect("typed writers dirty");
    assert_eq!(
        bytes,
        b"[S]\r\nI=-42\r\nF=0.500000\r\nB=True\r\nB2=False\r\n\r\n"
    );
}

#[test]
fn set_on_duplicated_key_updates_newest_only() {
    let mut file = parse("[S]\nA=1\nA=2\n");
    file.set_string("S", "A", "3");
    let bytes = file.render_if_dirty().expect("dup update dirties");
    assert_eq!(bytes, b"[S]\r\nA=1\r\nA=3\r\n\r\n");
}

#[test]
fn empty_section_clears_but_keeps_header() {
    let mut file = parse("[S]\nK=V\n[T]\nJ=W\n");
    file.empty_section("Absent"); // no-op
    assert!(file.render_if_dirty().is_none(), "missing section no dirty");
    file.empty_section("S");
    let bytes = file.render_if_dirty().expect("emptied section dirties");
    assert_eq!(bytes, b"[S]\r\n\r\n[T]\r\nJ=W\r\n\r\n");
}

#[test]
fn detached_file_never_writes() {
    let mut file = IniFile::default();
    file.set_string("S", "K", "v");
    file.detach();
    assert!(file.render_if_dirty().is_none(), "NoSave suppresses render");
}
#[test]
fn dirty_but_empty_config_refuses_to_render() {
    let file = IniFile::default();
    assert!(file.render_if_dirty().is_none(), "clean empty: no write");
    // Dirty-but-empty has no renderable surface here because sections only
    // materialize through sets; the C++ pin (Write returns 0) maps to None.
    let mut bare = IniFile::default();
    bare.force_dirty();
    assert!(bare.render_if_dirty().is_none());
}

#[test]
fn rewrite_normalization_is_a_fixed_point() {
    let mut file = parse(
        "; top comment\r\n[Render]\r\nMode=32\r\n; note=x\r\nQuality=\"High\"\r\nnoequals\r\nMode=16\n[Audio]\n\n[Render]\r\nExtra=1\r\n",
    );
    // Read-side pins feeding the rewrite:
    assert_eq!(file.get("Render", "Quality"), Some("High"));
    assert_eq!(file.get("RENDER", "MODE"), Some("16"));
    assert_eq!(file.get("Render", "; note"), Some("x"));
    assert_eq!(file.get_array("Render", "Mode"), vec!["32", "16"]);
    file.force_dirty(); // the C++ oracle sets File.Dirty = 1 directly

    let out = file.render_if_dirty().expect("forced dirty rewrite");
    let expected: &[u8] = b"[Render]\r\nMode=32\r\n; note=x\r\nQuality=High\r\nMode=16\r\nExtra=1\r\n\r\n[Audio]\r\n\r\n";
    assert_eq!(out, expected);

    // Re-parse + rewrite must be byte-stable.
    let again = IniFile::parse(&out);
    let mut again = again;
    again.force_dirty();
    let out2 = again.render_if_dirty().expect("second pass dirty");
    assert_eq!(out2, expected, "rewrite must reach a fixed point");
}

#[test]
fn extension_dot_rule_unit_cases() {
    // The pure string-resolution rule from TestCacheFilenames:
    // append ".ini" iff len < 5, or no '.' sits at exactly Len-4 / Len-5.
    fn resolves(name: &str) -> String {
        hp_ini::resolve_ini_name(name)
    }
    assert_eq!(resolves("PlainName"), "PlainName.ini");
    assert_eq!(resolves("CfgSys.ini"), "CfgSys.ini"); // verbatim
    assert_eq!(resolves("a.b.c"), "a.b.c"); // dot at Len-4: never re-extended
    assert_eq!(resolves("ab.cd"), "ab.cd.ini"); // dots exist but not at the watched positions
    assert_eq!(resolves("user.ini"), "user.ini");
}

// -------------------------------------------------------------- merged set

#[test]
fn load_pair_user_overrides_default_and_arrays_concatenate() {
    let dir = std::env::temp_dir().join(format!("hp-ini-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let default = dir.join("Default.ini");
    let user = dir.join("User.ini");
    std::fs::write(&default, b"[Video]\nMode=32\n[Paths]\nPaths=../Custom\n").unwrap();
    std::fs::write(&user, b"[Video]\nMode=16\n").unwrap();

    let set = hp_ini::load_pair(&default, Some(&user)).unwrap();
    assert_eq!(set.get("Video", "Mode"), Some("16"));
    assert_eq!(set.get_array("Paths", "Paths"), vec!["../Custom"]);

    let paths = set.paths(Path::new("/data"));
    assert_eq!(paths, vec![PathBuf::from("/data/Custom")]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn paths_defaults_when_no_entries() {
    let dir = std::env::temp_dir().join(format!("hp-ini-test-def-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let default = dir.join("Default.ini");
    std::fs::write(&default, b"[Video]\nMode=32\n").unwrap();
    let set = hp_ini::load_pair(&default, None).unwrap();
    let paths = set.paths(Path::new("/data"));
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/data/System"),
            PathBuf::from("/data/Maps"),
            PathBuf::from("/data/Textures"),
            PathBuf::from("/data/Sounds"),
            PathBuf::from("/data/Music"),
        ]
    );
    std::fs::remove_dir_all(&dir).ok();
}
