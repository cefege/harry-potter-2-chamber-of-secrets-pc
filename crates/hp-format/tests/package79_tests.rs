//! Ported contracts for ctests `compact_index` and `fstring_archive`
//! (Tests/AbiTests.cpp) plus package79 round-trip and property-grammar tests.

use hp_format::package79::{
    ByteCursor, PackageError, encode_compact_index, read_compact_index, read_fstring,
    write_compact_index, write_fstring,
};

fn round_trip_compact(value: i32, expected: &[u8]) {
    let mut out = Vec::new();
    write_compact_index(&mut out, value);
    assert_eq!(out, expected, "encode({value})");

    let decoded = read_compact_index(&mut ByteCursor::new(expected))
        .unwrap_or_else(|e| panic!("decode {expected:?}: {e}"));
    assert_eq!(decoded, value, "decode {expected:?}");
}

#[test]
fn compact_index_boundary_values() {
    round_trip_compact(0, &[0x00]);
    round_trip_compact(63, &[0x3f]);
    round_trip_compact(-63, &[0xbf]);
    round_trip_compact(64, &[0x40, 0x01]);
    round_trip_compact(-64, &[0xc0, 0x01]);
    round_trip_compact(8191, &[0x7f, 0x7f]);
    round_trip_compact(-8191, &[0xff, 0x7f]);
    round_trip_compact(8192, &[0x40, 0x80, 0x01]);
    round_trip_compact(-8192, &[0xc0, 0x80, 0x01]);
}
/// Width boundaries from Build/InputScriptSmoke.py's engine-encoder table.
#[test]
fn compact_index_wider_boundaries() {
    round_trip_compact(0x3FFF, &[0x7f, 0xff, 0x01]);
    round_trip_compact(0x400000, &[0x40, 0x80, 0x80, 0x04]);
}

#[test]
fn compact_index_extremes() {
    round_trip_compact(i32::MAX, &[0x7f, 0xff, 0xff, 0xff, 0x0f]);
    round_trip_compact(i32::MIN, &[0xc0, 0x80, 0x80, 0x80, 0x10]);
}

fn reject_compact(bytes: &[u8], want: PackageError) {
    match read_compact_index(&mut ByteCursor::new(bytes)) {
        Err(e) => assert_eq!(e, want, "input {bytes:?}"),
        Ok(v) => panic!("input {bytes:?}: expected rejection, got {v}"),
    }
}

#[test]
fn compact_index_rejects_non_canonical_and_overflow() {
    reject_compact(&[0x80], PackageError::NonCanonicalCompactIndex); // negative zero
    reject_compact(&[0x40, 0x00], PackageError::NonCanonicalCompactIndex); // long zero
    // Overflow: magnitude 1 << 31 is outside int32.
    reject_compact(
        &[0x40, 0x80, 0x80, 0x80, 0x10],
        PackageError::CompactIndexOutOfRange,
    );
}

#[test]
fn compact_index_rejects_truncated_forms() {
    reject_compact(&[0x40], PackageError::Truncated { need: 1, got: 1 });
    reject_compact(&[0x40, 0x80], PackageError::Truncated { need: 1, got: 2 });
    reject_compact(
        &[0x40, 0x80, 0x80],
        PackageError::Truncated { need: 1, got: 3 },
    );
    reject_compact(
        &[0x40, 0x80, 0x80, 0x80],
        PackageError::Truncated { need: 1, got: 4 },
    );
}

#[test]
fn fstring_archive_boundary_cases() {
    // SaveStringAndCheck cases from TestFStringArchive.
    let mut out = Vec::new();
    write_fstring(&mut out, "HP2");
    assert_eq!(out, [0x04, b'H', b'P', b'2', 0x00]);

    out.clear();
    write_fstring(&mut out, "A\u{03a9}");
    assert_eq!(out, [0x83, 0x41, 0x00, 0xa9, 0x03, 0x00, 0x00]);

    out.clear();
    write_fstring(&mut out, "\u{1f642}");
    assert_eq!(out, [0x83, 0x3d, 0xd8, 0x42, 0xde, 0x00, 0x00]);

    // Load + exact byte-count consumption + save-back round trip.
    let non_bmp = [0x83u8, 0x3d, 0xd8, 0x42, 0xde, 0x00, 0x00];
    let mut cur = ByteCursor::new(&non_bmp);
    let value = read_fstring(&mut cur).expect("non-BMP decode");
    assert_eq!(cur.position(), non_bmp.len());
    assert_eq!(value, "\u{1f642}");
    out.clear();
    write_fstring(&mut out, &value);
    assert_eq!(out, non_bmp);
}

#[test]
fn fstring_malformed_utf16_becomes_replacement_char() {
    let lone_high = [0x82u8, 0x00, 0xd8, 0x00, 0x00];
    let mut cur = ByteCursor::new(&lone_high);
    let value = read_fstring(&mut cur).expect("lossy decode");
    assert_eq!(cur.position(), lone_high.len());
    let chars: Vec<char> = value.chars().collect();
    assert_eq!(chars, ['\u{FFFD}']);
}

#[test]
fn fstring_truncated_utf16_is_loud() {
    // The C++ oracle (AbiTests.cpp:453-456) pins "load must error"; it
    // does not distinguish which loud variant fires.
    let truncated = [0x82u8, 0x41, 0x00];
    assert!(read_fstring(&mut ByteCursor::new(&truncated)).is_err());
}

#[test]
fn fstring_empty_and_ansi_edge_shapes() {
    // Zero-length payload decodes to the empty FString.
    assert_eq!(read_fstring(&mut ByteCursor::new(&[0x00])).unwrap(), "");

    let ansi = [0x02u8, b'X', 0x00];
    let mut cur = ByteCursor::new(&ansi);
    assert_eq!(read_fstring(&mut cur).unwrap(), "X");
    assert_eq!(cur.position(), 3);
}

#[test]
fn encoder_matches_reference_for_all_byte_patterns() {
    // Encoder/decoder agreement across every one-byte and a sample of
    // multi-byte magnitudes.
    for v in [
        0i32,
        1,
        -1,
        0x3F,
        -0x3F,
        0x40,
        -0x40,
        0x1F_FFFF,
        -(0x1F_FFFF),
        i32::MAX,
        i32::MIN,
    ] {
        let encoded = encode_compact_index(v);
        let decoded = read_compact_index(&mut ByteCursor::new(&encoded))
            .unwrap_or_else(|e| panic!("{v}: {e}"));
        assert_eq!(decoded, v);
        // Shortest form: no shorter encoding exists that decodes to v.
        if encoded.len() > 1 {
            let shorter = &encoded[..encoded.len() - 1];
            assert!(read_compact_index(&mut ByteCursor::new(shorter)).is_err());
        }
    }
}

// ---------------------------------------------------------------------------
// Archive reader/writer

use hp_format::package79::{
    ExportEntry, ImportEntry, Json, NameEncoding, NameEntry, ObjectPaths, PROPERTY_TYPE_BOOL,
    PROPERTY_TYPE_STR, PROPERTY_TYPE_STRUCT, PackageArchive, PropertyTag, TableDirectory,
    derive_function_terminal, read_package, read_property_tags, write_package, write_property_tag,
    write_property_terminator,
};
use hp_format::package79::{GenerationEntry, Guid};

/// Assemble a package from its structural parts and assert that reading it
/// and writing it back reproduces the exact input bytes.
fn assert_round_trips(bytes: &[u8]) -> PackageArchive {
    let archive = read_package(bytes).unwrap_or_else(|e| panic!("read failed: {e}"));
    let mut out = Vec::with_capacity(bytes.len());
    write_package(&archive, &mut out).unwrap_or_else(|e| panic!("write failed: {e}"));
    assert_eq!(out, bytes);
    archive
}

struct Parts {
    version: i32,
    names: Vec<NameEntry>,
    name_bytes: Vec<u8>,
    imports: Vec<ImportEntry>,
    import_bytes: Vec<u8>,
    exports: Vec<ExportEntry>,
    export_bytes: Vec<u8>,
    payload: Vec<u8>,
    tables: TableDirectory,
    header_gap: Vec<u8>,
}

/// Assemble a complete package image from its parts, computing offsets the
/// way the reference writer would.
fn build_package(parts: &Parts) -> Vec<u8> {
    let mut bytes = Vec::new();
    let version_word = (parts.version as u32) & 0xFFFF;
    bytes.extend_from_slice(&0x9E2A83C1u32.to_le_bytes());
    bytes.extend_from_slice(&version_word.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes()); // PKG_ flags
    let counts_base = bytes.len();
    for _ in 0..6 {
        bytes.extend_from_slice(&0i32.to_le_bytes());
    }
    match &parts.tables {
        TableDirectory::Generations { guid, entries } => {
            for word in [guid.a, guid.b, guid.c, guid.d] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes.extend_from_slice(&(entries.len() as i32).to_le_bytes());
            for entry in entries {
                bytes.extend_from_slice(&entry.export_count.to_le_bytes());
                bytes.extend_from_slice(&entry.name_count.to_le_bytes());
            }
        }
        TableDirectory::Heritage { count, offset } => {
            bytes.extend_from_slice(&count.to_le_bytes());
            bytes.extend_from_slice(&offset.to_le_bytes());
            bytes.extend_from_slice(&parts.header_gap);
        }
    }
    let name_offset = bytes.len() as i32;
    bytes.extend_from_slice(&parts.name_bytes);
    let import_offset = (bytes.len() + parts.payload.len()) as i32;
    bytes.extend_from_slice(&parts.payload);
    bytes.extend_from_slice(&parts.import_bytes);
    let export_offset = bytes.len() as i32;
    bytes.extend_from_slice(&parts.export_bytes);

    for (slot, value) in [
        parts.names.len() as i32,
        name_offset,
        parts.exports.len() as i32,
        export_offset,
        parts.imports.len() as i32,
        import_offset,
    ]
    .into_iter()
    .enumerate()
    {
        let position = counts_base + slot * 4;
        bytes[position..position + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn ansi_name(text: &str, flags: u32) -> NameEntry {
    NameEntry {
        text: text.to_string(),
        flags,
        encoding: NameEncoding::Ansi,
    }
}

fn serialize_name(entry: &NameEntry) -> Vec<u8> {
    let mut out = Vec::new();
    hp_format::package79::write_fstring(&mut out, &entry.text);
    out.extend_from_slice(&entry.flags.to_le_bytes());
    out
}

#[test]
fn v79_package_with_generations_round_trips() {
    let names = vec![ansi_name("None", 0), ansi_name("Foo", 0x07001000)];
    let mut name_bytes = Vec::new();
    for entry in &names {
        name_bytes.extend(serialize_name(entry));
    }

    // One import: class package index 0, class index 0, outer none, "Foo".
    let mut import_bytes = Vec::new();
    hp_format::package79::write_compact_index(&mut import_bytes, 0);
    hp_format::package79::write_compact_index(&mut import_bytes, 0);
    import_bytes.extend_from_slice(&0i32.to_le_bytes());
    hp_format::package79::write_compact_index(&mut import_bytes, 1);

    let payload: Vec<u8> = (0u8..=15).collect();
    // Layout: fixed header (12) + six i32 slots (24) + GUID/gen table (28).
    let names_end = 12 + 24 + 28 + name_bytes.len();
    let mut export_bytes = Vec::new();
    hp_format::package79::write_compact_index(&mut export_bytes, -1); // class
    hp_format::package79::write_compact_index(&mut export_bytes, 0); // super
    export_bytes.extend_from_slice(&0i32.to_le_bytes()); // outer = package root
    hp_format::package79::write_compact_index(&mut export_bytes, 1); // "Foo"
    export_bytes.extend_from_slice(&0x0000_0400u32.to_le_bytes());
    hp_format::package79::write_compact_index(&mut export_bytes, payload.len() as i32);
    hp_format::package79::write_compact_index(&mut export_bytes, names_end as i32);

    let bytes = build_package(&Parts {
        version: 79,
        names,
        name_bytes,
        imports: vec![ImportEntry {
            class_package_index: 0,
            class_name_index: 0,
            outer_ref: 0,
            object_name_index: 1,
        }],
        import_bytes,
        exports: vec![ExportEntry {
            class_ref: -1,
            super_ref: 0,
            outer_ref: 0,
            object_name_index: 1,
            object_flags: 0x0000_0400,
            serial_size: payload.len() as i32,
            serial_offset: Some(names_end as i32),
        }],
        export_bytes,
        payload,
        tables: TableDirectory::Generations {
            guid: Guid {
                a: 1,
                b: 2,
                c: 3,
                d: 4,
            },
            entries: vec![GenerationEntry {
                export_count: 1,
                name_count: 2,
            }],
        },
        header_gap: Vec::new(),
    });

    let archive = read_package(&bytes).expect("v79 synthetic must parse");
    assert_eq!(archive.summary.version, 79);
    assert_eq!(
        archive.export_payload(0).unwrap(),
        &(0u8..=15).collect::<Vec<u8>>()
    );
    assert_round_trips(&bytes);
}

/// The two v61 prototype files carry heritage tables inside the gap between
/// the summary and the name table; the writer must reproduce those gap bytes
/// verbatim.
#[test]
fn v61_heritage_gap_round_trips() {
    let names = vec![NameEntry {
        text: "None".into(),
        flags: 0x07001000,
        encoding: NameEncoding::Ansi,
    }];
    // Pre-64 form: NUL-terminated ANSI without a length prefix.
    let mut name_bytes = b"None\0".to_vec();
    name_bytes.extend_from_slice(&0x07001000u32.to_le_bytes());

    let gap: Vec<u8> = (0x20..0x30).collect(); // stand-in heritage table
    let names_end = 12 + 24 + 8 + gap.len() + name_bytes.len();

    let mut export_bytes = Vec::new();
    hp_format::package79::write_compact_index(&mut export_bytes, 0); // class none
    hp_format::package79::write_compact_index(&mut export_bytes, 0);
    export_bytes.extend_from_slice(&0i32.to_le_bytes());
    hp_format::package79::write_compact_index(&mut export_bytes, 0);
    export_bytes.extend_from_slice(&0i32.to_le_bytes()); // RF_LoadForClient etc.
    hp_format::package79::write_compact_index(&mut export_bytes, 0); // zero size

    let bytes = build_package(&Parts {
        version: 61,
        names,
        name_bytes,
        imports: vec![],
        import_bytes: Vec::new(),
        exports: vec![ExportEntry {
            class_ref: 0,
            super_ref: 0,
            outer_ref: 0,
            object_name_index: 0,
            object_flags: 0,
            serial_size: 0,
            serial_offset: None,
        }],
        export_bytes,
        payload: Vec::new(),
        tables: TableDirectory::Heritage {
            count: 1,
            offset: 44,
        },
        header_gap: gap,
    });

    let archive = read_package(&bytes).expect("v61 synthetic must parse");
    assert_eq!(archive.summary.version, 61);
    assert_eq!(archive.names_end(), names_end);
    assert_round_trips(&bytes);
}

#[test]
fn unsupported_versions_and_bad_tags_rejected() {
    let make = |version_word: u32| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x9E2A83C1u32.to_le_bytes());
        bytes.extend_from_slice(&version_word.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 36]);
        bytes
    };
    // A bare summary is not a complete package: it must fail somewhere.
    assert!(read_package(&make(60)).is_err());
    let mut bad_tag = Vec::new();
    bad_tag.extend_from_slice(&0xDEADBEEFu32.to_le_bytes());
    bad_tag.extend_from_slice(&[0u8; 36]);
    match read_package(&bad_tag) {
        Err(PackageError::BadTag { got: 0xDEAD_BEEF }) => {}
        other => panic!("bad tag should be rejected, got {other:?}"),
    }
    let mut bad_version = Vec::new();
    bad_version.extend_from_slice(&0x9E2A83C1u32.to_le_bytes());
    bad_version.extend_from_slice(&59u32.to_le_bytes());
    bad_version.extend_from_slice(&[0u8; 36]);
    match read_package(&bad_version) {
        Err(PackageError::UnsupportedVersion { got: 59 }) => {}
        other => panic!("v59 should be unsupported, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Tagged property grammar (cross-reference Build/smoke_maps.py:187-244)

fn property_test_names() -> Vec<NameEntry> {
    ["None", "MyInt", "MyStr", "MyStruct", "MyBool", "MyArray"]
        .iter()
        .map(|text| ansi_name(text, 0x07001000))
        .collect()
}

fn tag(name_index: i32, info_kind: u8, size_code: u8, array_flag: bool) -> PropertyTag {
    PropertyTag {
        name_index,
        kind: info_kind,
        size_code,
        array_flag,
        struct_name_index: None,
        array_index: None,
        payload: Vec::new(),
    }
}

#[test]
fn property_list_round_trips_through_grammar() {
    let names = property_test_names();
    let mut bytes = Vec::new();

    // Int property, implicit 4-byte size.
    let int_tag = PropertyTag {
        name_index: 1,
        payload: 12345i32.to_le_bytes().to_vec(),
        ..tag(1, 1, 0x20, false)
    };
    // String property with explicit u16 size prefix.
    let mut str_payload = Vec::new();
    write_fstring(&mut str_payload, "Hello");
    let str_tag = PropertyTag {
        name_index: 2,
        size_code: 0x60,
        payload: str_payload.clone(),
        ..tag(2, PROPERTY_TYPE_STR, 0x60, false)
    };
    // Struct property carrying a struct-name index.
    let struct_tag = PropertyTag {
        name_index: 3,
        struct_name_index: Some(3),
        payload: vec![1, 2, 3, 4],
        ..tag(3, PROPERTY_TYPE_STRUCT, 0x20, false)
    };
    // Bool property: the 0x80 bit is the value, never an array index.
    let bool_tag = PropertyTag {
        name_index: 4,
        array_flag: true,
        payload: vec![1],
        ..tag(4, PROPERTY_TYPE_BOOL, 0x00, true)
    };
    // Array element: array flag decodes a compact element index.
    let array_tag = PropertyTag {
        name_index: 5,
        array_flag: true,
        array_index: Some(7),
        payload: vec![9; 12],
        ..tag(5, 6, 0x30, true)
    };

    for property in [&int_tag, &str_tag, &struct_tag, &bool_tag, &array_tag] {
        write_property_tag(&mut bytes, property);
    }
    write_property_terminator(&mut bytes, 0); // "None"

    let mut cursor = ByteCursor::new(&bytes);
    let decoded = read_property_tags(&mut cursor, &names).expect("property list must parse");
    assert_eq!(
        decoded,
        vec![int_tag, str_tag, struct_tag, bool_tag, array_tag]
    );
    assert_eq!(cursor.position(), bytes.len());

    // Re-encoding must reproduce the exact input bytes.
    let mut reencoded = Vec::new();
    for property in &decoded {
        write_property_tag(&mut reencoded, property);
    }
    write_property_terminator(&mut reencoded, 0);
    assert_eq!(reencoded, bytes);
}

#[test]
fn property_rejects_bad_indices_and_negative_sizes() {
    let names = property_test_names();

    // Kind 10 (Struct) accepts a struct-name index before the size.
    let mut struct_tag_bytes = encode_compact_index(3).to_vec();
    struct_tag_bytes.push(PROPERTY_TYPE_STRUCT | 0x20);
    struct_tag_bytes.extend_from_slice(&encode_compact_index(2));
    struct_tag_bytes.extend_from_slice(&[0, 0, 0, 0]);
    write_property_terminator(&mut struct_tag_bytes, 0);
    let mut cursor = ByteCursor::new(&struct_tag_bytes);
    assert!(read_property_tags(&mut cursor, &names).is_ok());

    // Explicit i32 size prefix must reject negative sizes.
    let mut negative_size = encode_compact_index(1).to_vec();
    negative_size.push(1 | 0x70); // kind 1, explicit i32 size
    negative_size.extend_from_slice(&(-1i32).to_le_bytes());
    match read_property_tags(&mut ByteCursor::new(&negative_size), &names) {
        Err(_) => {}
        Ok(value) => panic!("negative property size must be rejected, got {value:?}"),
    }
}
// ---------------------------------------------------------------------------
// Function terminal derivation + object paths

#[test]
fn function_terminal_non_net_and_net_forms() {
    #[allow(clippy::needless_update)]
    let mut entry = ExportEntry {
        class_ref: 0,
        super_ref: 0,
        outer_ref: 0,
        object_name_index: 0,
        object_flags: 0,
        serial_offset: Some(0),
        serial_size: 0,
    };

    // Non-net trailer: native index 5, precedence 1, native flag set.
    let mut data = vec![0u8; 4]; // body
    data.extend_from_slice(&5u16.to_le_bytes());
    data.push(1);
    data.extend_from_slice(&(0x400u32).to_le_bytes());
    entry.serial_size = data.len() as i32;
    let terminal = derive_function_terminal(&data, &entry)
        .unwrap_or_else(|error| panic!("non-net derives failed: {error}"));
    assert_eq!(terminal.native_index, 5);
    assert_eq!(terminal.function_flags, 0x400);

    // Net trailer adds RepOffset; NET + RELIABLE + NATIVE bits set.
    let mut net_data = vec![0u8; 4];
    net_data.extend_from_slice(&5u16.to_le_bytes());
    net_data.push(2);
    net_data.extend_from_slice(
        &((hp_format::package79::FUNCTION_FLAG_NET
            | hp_format::package79::FUNCTION_FLAG_NET_RELIABLE
            | hp_format::package79::FUNCTION_FLAG_NATIVE)
            .to_le_bytes()),
    );
    net_data.extend_from_slice(&0x1234u16.to_le_bytes());
    entry.serial_size = net_data.len() as i32;
    let terminal = derive_function_terminal(&net_data, &entry).expect("net derives");
    assert_eq!(terminal.native_index, 5);
    assert_eq!(
        terminal.function_flags & hp_format::package79::FUNCTION_FLAG_NET,
        hp_format::package79::FUNCTION_FLAG_NET
    );

    // Neither window consistent -> loud zero-candidate rejection.
    let inconsistent = vec![0xFFu8; 12];
    entry.serial_size = inconsistent.len() as i32;
    match derive_function_terminal(&inconsistent, &entry) {
        Err(hp_format::package79::PackageError::AmbiguousFunctionTerminal { candidates }) => {
            assert_eq!(candidates, 0)
        }
        other => panic!("expected zero-candidate rejection, got {other:?}"),
    }
}

#[test]
fn object_paths_match_reference_conventions() {
    let names: Vec<NameEntry> = ["Core", "Package", "Engine", "Sound", "Root", "Child"]
        .iter()
        .map(|text| ansi_name(text, 0))
        .collect();
    let imports = vec![ImportEntry {
        class_package_index: 0,
        class_name_index: 1,
        outer_ref: 0,
        object_name_index: 3,
    }];
    let exports = vec![
        ExportEntry {
            class_ref: -1,
            super_ref: 0,
            outer_ref: 0,
            object_name_index: 4,
            object_flags: 0,
            serial_size: 0,
            serial_offset: None,
        },
        ExportEntry {
            class_ref: 0,
            super_ref: 0,
            outer_ref: 1,
            object_name_index: 5,
            object_flags: 0,
            serial_size: 0,
            serial_offset: None,
        },
    ];
    let paths = ObjectPaths::new("MyPack", &names, &imports, &exports);
    assert_eq!(paths.import_path(0).unwrap(), "Sound");
    assert_eq!(paths.export_path(1).unwrap(), "MyPack.Root.Child");
    assert_eq!(paths.ref_path(0).unwrap(), None);
    assert_eq!(paths.ref_path(-1).unwrap().as_deref(), Some("Sound"));
    assert_eq!(paths.ref_path(1).unwrap().as_deref(), Some("MyPack.Root"));
}

// ---------------------------------------------------------------------------
// Canonical JSON emitter parity with python json.dumps(indent=2, sort_keys)

#[test]
fn canonical_json_matches_python_formatting() {
    let mut inner = std::collections::BTreeMap::new();
    inner.insert("b".to_string(), Json::Arr(vec![Json::Int(1), Json::Int(2)]));
    inner.insert("a".to_string(), Json::str("quote\" back\\slash\n"));
    let document = Json::obj(vec![
        ("z", Json::Obj(inner)),
        ("empty", Json::Arr(Vec::new())),
        ("n", Json::Null),
        ("neg", Json::Int(-42)),
        ("ctl", Json::str("\u{01}\t")),
    ]);
    let expected = concat!(
        "{\n",
        "  \"ctl\": \"\\u0001\\t\",\n",
        "  \"empty\": [],\n",
        "  \"n\": null,\n",
        "  \"neg\": -42,\n",
        "  \"z\": {\n",
        "    \"a\": \"quote\\\" back\\\\slash\\n\",\n",
        "    \"b\": [\n",
        "      1,\n",
        "      2\n",
        "    ]\n",
        "  }\n",
        "}\n",
    );
    assert_eq!(document.to_canonical(), expected);
}

// ---------------------------------------------------------------------------
// Whole-tree round trip over the prototype root

#[test]
fn round_trips_every_prototype_package() {
    fn collect(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
        let mut entries: std::collections::BTreeMap<String, std::path::PathBuf> =
            std::collections::BTreeMap::new();
        for entry in dir.read_dir().expect("data root readable").flatten() {
            entries.insert(
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            );
        }
        for (_name, path) in entries {
            if path.is_dir() {
                collect(root, &path, out);
            } else if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("cargo env");
    let root = std::path::Path::new(&manifest_dir).join("../../HarryPotter2/Unreal");
    if !root.join("System/Default.ini").is_file() {
        println!("blocked: prototype tree absent (no game data root)");
        return;
    }
    let root = root.canonicalize().expect("prototype tree present");
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();

    let mut tagged = 0usize;
    let mut findings: Vec<String> = Vec::new();
    for relative in &files {
        let Ok(data) = std::fs::read(root.join(relative)) else {
            findings.push(format!("{relative}: unreadable"));
            continue;
        };
        if data.len() < 4
            || u32::from_le_bytes([data[0], data[1], data[2], data[3]])
                != hp_format::package79::PACKAGE_TAG
        {
            continue; // not a UE1 package; outside this reader's contract
        }
        tagged += 1;
        match read_package(&data) {
            Ok(archive) => {
                let mut out = Vec::with_capacity(data.len());
                if let Err(error) = write_package(&archive, &mut out) {
                    findings.push(format!("{relative}: write failed: {error}"));
                } else if out != data {
                    findings.push(format!(
                        "{relative}: round-trip mismatch ({} in, {} out)",
                        data.len(),
                        out.len()
                    ));
                }
            }
            Err(error) => findings.push(format!("{relative}: read failed: {error}")),
        }
    }

    // Coverage pin: every package file under the prototype root as of the
    // Phase-1 freeze (v61 x2 heritage, v68 x2, v69, v76 x77, v79 x132).
    assert_eq!(tagged, 214, "unexpected package census");
    assert!(
        findings.is_empty(),
        "round-trip findings:\n{}",
        findings.join("\n")
    );
}
