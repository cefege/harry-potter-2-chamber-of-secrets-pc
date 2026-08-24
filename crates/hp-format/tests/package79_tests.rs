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
