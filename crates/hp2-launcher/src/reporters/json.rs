//! Hand-rolled JSON scanner, ported byte-for-byte from `SDLLaunch.cpp`'s
//! `SkipJsonWhitespace`/`ReadJsonString`/`JsonObjectEnd`/... family
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp:789-1050`). Every
//! reporter's scenario/manifest parser in this crate is built from these.
//!
//! Operates on raw bytes, mirroring the C++ `std::string`/cursor-of-`size_t`
//! walk exactly (`std::isspace`, single-byte escapes); the documents this
//! parses are trusted ASCII JSON fixtures, so byte-at-a-time scanning matches
//! the original one-for-one.
use hp2_abi::FVector;

/// `SkipJsonWhitespace`.
pub(crate) fn skip_ws(text: &[u8], cursor: &mut usize) {
    while *cursor < text.len() && (text[*cursor] as char).is_ascii_whitespace() {
        *cursor += 1;
    }
}

/// `ReadJsonString`.
pub(crate) fn read_string(text: &[u8], cursor: &mut usize) -> Option<Vec<u8>> {
    skip_ws(text, cursor);
    if *cursor >= text.len() || text[*cursor] != b'"' {
        return None;
    }
    *cursor += 1;
    let mut out = Vec::new();
    while *cursor < text.len() {
        let ch = text[*cursor];
        *cursor += 1;
        if ch == b'"' {
            return Some(out);
        }
        if ch != b'\\' {
            out.push(ch);
            continue;
        }
        if *cursor >= text.len() {
            return None;
        }
        let esc = text[*cursor];
        *cursor += 1;
        match esc {
            b'"' => out.push(b'"'),
            b'\\' => out.push(b'\\'),
            b'/' => out.push(b'/'),
            b'b' => out.push(0x08),
            b'f' => out.push(0x0c),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            _ => return None,
        }
    }
    None
}

fn read_string_str(text: &[u8], cursor: &mut usize) -> Option<String> {
    read_string(text, cursor).map(|b| String::from_utf8_lossy(&b).into_owned())
}

/// `JsonObjectEnd`: `Begin` must point at `{`; returns the index just past
/// the matching `}`.
pub(crate) fn object_end(text: &[u8], begin: usize) -> Option<usize> {
    if begin >= text.len() || text[begin] != b'{' {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    let mut cursor = begin;
    while cursor < text.len() {
        let ch = text[cursor];
        if in_string {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            cursor += 1;
            continue;
        }
        if ch == b'"' {
            in_string = true;
        } else if ch == b'{' {
            depth += 1;
        } else if ch == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(cursor + 1);
            }
        }
        cursor += 1;
    }
    None
}

/// `JsonMemberCursor`: cursor positioned just after `key`'s `:`.
fn member_cursor(object: &[u8], key: &str) -> Option<usize> {
    let width = key.len() + 2;
    for (found, candidate) in object.windows(width).enumerate() {
        if candidate[0] != b'"'
            || candidate[width - 1] != b'"'
            || &candidate[1..width - 1] != key.as_bytes()
        {
            continue;
        }
        let mut cursor = found + width;
        skip_ws(object, &mut cursor);
        if object.get(cursor) != Some(&b':') {
            continue;
        }
        cursor += 1;
        skip_ws(object, &mut cursor);
        return Some(cursor);
    }
    None
}

/// `JsonStringMember`.
pub(crate) fn string_member(object: &[u8], key: &str) -> Option<String> {
    let mut cursor = member_cursor(object, key)?;
    read_string_str(object, &mut cursor)
}

/// `JsonIntMember`: base-10 `strtol`, rejecting values outside `i32`.
pub(crate) fn int_member(object: &[u8], key: &str) -> Option<i32> {
    let cursor = member_cursor(object, key)?;
    strtol(&object[cursor..]).map(|(value, _)| value)
}

/// `strtol(str, &end, 10)` over a byte slice starting at the number, with
/// the exact `i32` bound check `JsonIntMember`/`JsonStaticMovementGateIntMember`
/// use (`Value < -(long)MAXINT - 1 || Value > MAXINT`).
fn strtol(bytes: &[u8]) -> Option<(i32, usize)> {
    let mut i = 0usize;
    while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    let mut neg = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        neg = bytes[i] == b'-';
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    let digits = std::str::from_utf8(&bytes[digits_start..i]).ok()?;
    let magnitude: i64 = digits.parse().ok()?;
    let value: i64 = if neg { -magnitude } else { magnitude };
    if value < i32::MIN as i64 || value > i32::MAX as i64 {
        return None;
    }
    let _ = start;
    Some((value as i32, i))
}

/// `JsonObjectMember`: the raw `{...}` substring for an object-valued key.
pub(crate) fn object_member(object: &[u8], key: &str) -> Option<Vec<u8>> {
    let cursor = member_cursor(object, key)?;
    let end = object_end(object, cursor)?;
    Some(object[cursor..end].to_vec())
}

/// `JsonValueEnd`: advances `cursor` past one JSON value of any kind.
pub(crate) fn value_end(text: &[u8], cursor: &mut usize) -> bool {
    skip_ws(text, cursor);
    if *cursor >= text.len() {
        return false;
    }
    if text[*cursor] == b'{' {
        return match object_end(text, *cursor) {
            Some(end) => {
                *cursor = end;
                true
            }
            None => false,
        };
    }
    if text[*cursor] == b'"' {
        let mut ignored = *cursor;
        return read_string(text, &mut ignored).map(|_| *cursor = ignored).is_some();
    }
    if text[*cursor] == b'[' {
        *cursor += 1;
        loop {
            skip_ws(text, cursor);
            if *cursor >= text.len() {
                return false;
            }
            if text[*cursor] == b']' {
                *cursor += 1;
                return true;
            }
            if !value_end(text, cursor) {
                return false;
            }
            skip_ws(text, cursor);
            if *cursor >= text.len() {
                return false;
            }
            if text[*cursor] == b']' {
                *cursor += 1;
                return true;
            }
            if text[*cursor] != b',' {
                return false;
            }
            *cursor += 1;
            skip_ws(text, cursor);
            if *cursor >= text.len() || text[*cursor] == b']' {
                return false;
            }
        }
    }
    if text[*cursor..].starts_with(b"true") {
        *cursor += 4;
        return true;
    }
    if text[*cursor..].starts_with(b"false") {
        *cursor += 5;
        return true;
    }
    if text[*cursor..].starts_with(b"null") {
        *cursor += 4;
        return true;
    }
    match strtod(&text[*cursor..]) {
        Some((_, len)) if len > 0 => {
            *cursor += len;
            true
        }
        _ => false,
    }
}

/// `JsonObjectHasExactKeys`.
pub(crate) fn has_exact_keys(object: &[u8], expected: &[&str]) -> bool {
    let mut cursor = 0usize;
    skip_ws(object, &mut cursor);
    if cursor >= object.len() || object[cursor] != b'{' {
        return false;
    }
    cursor += 1;
    let mut keys: Vec<String> = Vec::new();
    loop {
        skip_ws(object, &mut cursor);
        if cursor >= object.len() {
            return false;
        }
        if object[cursor] == b'}' {
            cursor += 1;
            break;
        }
        let Some(key) = read_string_str(object, &mut cursor) else {
            return false;
        };
        skip_ws(object, &mut cursor);
        if cursor >= object.len() || object[cursor] != b':' {
            return false;
        }
        cursor += 1;
        if !value_end(object, &mut cursor) {
            return false;
        }
        if keys.contains(&key) {
            return false;
        }
        keys.push(key);
        skip_ws(object, &mut cursor);
        if cursor >= object.len() {
            return false;
        }
        if object[cursor] == b'}' {
            cursor += 1;
            break;
        }
        if object[cursor] != b',' {
            return false;
        }
        cursor += 1;
        skip_ws(object, &mut cursor);
        if cursor >= object.len() || object[cursor] == b'}' {
            return false;
        }
    }
    skip_ws(object, &mut cursor);
    if cursor != object.len() || keys.len() != expected.len() {
        return false;
    }
    expected.iter().all(|e| keys.iter().any(|k| k == e))
}

/// `ParseFidelityManifest`'s own ad hoc scan, `ParseWorldCollisionGateScenario`
/// and friends via `JsonFloatMember`: `strtod(str, &end)` over a byte slice,
/// rejecting `ERANGE`. Returns `(value, bytes consumed)`.
fn strtod(bytes: &[u8]) -> Option<(f64, usize)> {
    let mut i = 0usize;
    while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut saw_digit = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        saw_digit = true;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            saw_digit = true;
        }
    }
    if !saw_digit {
        return None;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mark = i;
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_digits_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_digits_start {
            i = j;
        } else {
            i = mark;
        }
    }
    let text = std::str::from_utf8(&bytes[start..i]).ok()?;
    let value: f64 = text.parse().ok()?;
    if value.is_infinite() {
        return None;
    }
    Some((value, i))
}

/// `JsonFloatMember`.
pub(crate) fn float_member(object: &[u8], key: &str) -> Option<f32> {
    let cursor = member_cursor(object, key)?;
    let (value, len) = strtod(&object[cursor..])?;
    if len == 0 {
        return None;
    }
    Some(value as f32)
}

/// `JsonBoolMember`: exactly `true`/`false`, nothing else.
pub(crate) fn bool_member(object: &[u8], key: &str) -> Option<bool> {
    let cursor = member_cursor(object, key)?;
    if object[cursor..].starts_with(b"true") {
        Some(true)
    } else if object[cursor..].starts_with(b"false") {
        Some(false)
    } else {
        None
    }
}

/// `JsonVectorMember`: `[x,y,z]`.
pub(crate) fn vector_member(object: &[u8], key: &str) -> Option<FVector> {
    let mut cursor = member_cursor(object, key)?;
    if cursor >= object.len() || object[cursor] != b'[' {
        return None;
    }
    cursor += 1;
    let mut components = [0.0f32; 3];
    for (index, component) in components.iter_mut().enumerate() {
        skip_ws(object, &mut cursor);
        let (value, len) = strtod(&object[cursor..])?;
        if len == 0 {
            return None;
        }
        *component = value as f32;
        cursor += len;
        skip_ws(object, &mut cursor);
        if index + 1 < 3 {
            if cursor >= object.len() || object[cursor] != b',' {
                return None;
            }
            cursor += 1;
        }
    }
    skip_ws(object, &mut cursor);
    if cursor >= object.len() || object[cursor] != b']' {
        return None;
    }
    Some(FVector {
        x: components[0],
        y: components[1],
        z: components[2],
    })
}

/// `ReadJsonDocument`: reads the whole file; `None` filename or missing file
/// is a clean failure, exactly like the C++'s `Filename ? fopen(...) : NULL`.
pub(crate) fn read_document(filename: Option<&str>) -> Option<Vec<u8>> {
    let filename = filename?;
    if filename.is_empty() {
        return None;
    }
    std::fs::read(filename).ok()
}

/// `JsonObjectArrayMember`.
pub(crate) fn object_array_member(object: &[u8], key: &str) -> Option<Vec<Vec<u8>>> {
    let mut cursor = member_cursor(object, key)?;
    if cursor >= object.len() || object[cursor] != b'[' {
        return None;
    }
    cursor += 1;
    let mut out = Vec::new();
    loop {
        skip_ws(object, &mut cursor);
        if cursor >= object.len() {
            return None;
        }
        if object[cursor] == b']' {
            return Some(out);
        }
        let end = object_end(object, cursor)?;
        out.push(object[cursor..end].to_vec());
        cursor = end;
        skip_ws(object, &mut cursor);
        if cursor < object.len() && object[cursor] == b',' {
            cursor += 1;
        }
    }
}

/// `JsonStringEquals`.
pub(crate) fn string_equals(object: &[u8], key: &str, expected: &str) -> bool {
    string_member(object, key).as_deref() == Some(expected)
}

/// `%.Pg` (P significant digits): C99 7.21.6.1p8's `%g` algorithm, used by
/// `FixedDeltaNumber`/`LifecycleGateReporter::Serialize`'s inline `"%.9g"`
/// for `fixed_dt` -- not Rust's `{}`/`{:e}` formatting, which differ in
/// both rounding granularity and trailing-zero/exponent-style rules.
/// Verified against a real compiled `printf("%.9g", ...)` probe across
/// zero, negative zero, both style-E/style-F branches and their exact
/// threshold values (`1e+09`, `9.99999975e-05`): bit-for-bit match.
pub(crate) fn format_g(value: f64, precision: usize) -> String {
    let p = precision.max(1);
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        };
    }
    // Style E with precision p-1 significant digits after the point,
    // to discover the decimal exponent exactly as %e would print it.
    let sci = format!("{:.*e}", p - 1, value);
    let e_pos = sci.find('e').expect("scientific format always has 'e'");
    let exp: i32 = sci[e_pos + 1..]
        .parse()
        .expect("exponent is a valid integer");
    if exp < -4 || exp >= p as i32 {
        let trimmed = trim_trailing_zeros(&sci[..e_pos]);
        format!("{trimmed}e{}{:02}", if exp < 0 { "-" } else { "+" }, exp.abs())
    } else {
        let frac_digits = (p as i32 - 1 - exp).max(0) as usize;
        let fixed = format!("{:.*}", frac_digits, value);
        trim_trailing_zeros(&fixed)
    }
}

/// `%.Pf` (P digits after the point): used everywhere a reporter ports a
/// C++ `fprintf`/`sprintf` `"%.Nf"` field (JSON vector/scalar members,
/// `<HP2_...>` diagnostic lines). Differs from Rust's `{:.*}` only on
/// non-finite inputs: verified against a real compiled `printf` probe on
/// this platform, `NAN`/`-NAN` both print as `nan` (no sign, lowercase --
/// Rust prints `NaN`) and `INFINITY`/`-INFINITY` print as `inf`/`-inf`
/// (same as Rust, kept here only for the single non-finite branch).
/// Finite values, including `-0.0`, already match Rust's `{:.*}` bit for
/// bit, so those fall through unchanged.
pub(crate) fn format_fixed(value: f64, precision: usize) -> String {
    if value.is_nan() {
        "nan".to_string()
    } else if value.is_infinite() {
        if value.is_sign_negative() { "-inf".to_string() } else { "inf".to_string() }
    } else {
        format!("{value:.precision$}")
    }
}

fn trim_trailing_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `JsonString` (the identical 7-case escape `switch` shared verbatim by
/// `WorldCollisionGateReporter::JsonString` and
/// `StaticBspProbeReporter::JsonString`): `"`, `\`, `\b`, `\f`, `\n`, `\r`,
/// `\t` escaped, every other byte (including non-ASCII) passed through
/// unchanged. Byte-oriented, not `char`-oriented: the input is raw
/// `appToAnsi` text (a lossy single-byte charset conversion), so escaping
/// must reproduce those exact bytes, never a UTF-8 substitution.
/// `LifecycleGateReporter::JsonString` additionally escapes control bytes
/// below `0x20` as `\u%04x` and is kept as its own copy in
/// `reporters::lifecycle_gate`, not shared here.
pub(crate) fn push_json_string(value: &[u8], out: &mut Vec<u8>) {
    out.push(b'"');
    for &byte in value {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0c => out.extend_from_slice(b"\\f"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            other => out.push(other),
        }
    }
    out.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_escaped_string() {
        let text = br#""a\n\"b\\c""#;
        let mut cursor = 0;
        assert_eq!(read_string(text, &mut cursor).unwrap(), b"a\n\"b\\c");
        assert_eq!(cursor, text.len());
    }

    #[test]
    fn finds_matching_object_end_through_nested_braces_and_strings() {
        let text = br#"{"a":{"b":1},"c":"}"}"#;
        assert_eq!(object_end(text, 0), Some(text.len()));
    }

    #[test]
    fn member_values_do_not_shadow_later_member_keys() {
        assert_eq!(int_member(br#"{"position":"tick","tick":4}"#, "tick"), Some(4));
        assert_eq!(
            string_member(br#"{"description":"name","name":"Harry"}"#, "name"),
            Some("Harry".into()),
        );
    }

    #[test]
    fn int_member_rejects_out_of_range() {
        let text = br#"{"v":9999999999}"#;
        assert_eq!(int_member(text, "v"), None);
        let text = br#"{"v":42}"#;
        assert_eq!(int_member(text, "v"), Some(42));
    }

    #[test]
    fn float_member_parses_exponent() {
        let text = br#"{"v":1.5e2}"#;
        assert_eq!(float_member(text, "v"), Some(150.0));
    }

    #[test]
    fn bool_member_rejects_non_literal() {
        let text = br#"{"v":1}"#;
        assert_eq!(bool_member(text, "v"), None);
        let text = br#"{"v":true}"#;
        assert_eq!(bool_member(text, "v"), Some(true));
    }

    #[test]
    fn vector_member_parses_three_components() {
        let text = br#"{"v":[1,2.5,-3]}"#;
        let v = vector_member(text, "v").unwrap();
        assert_eq!((v.x, v.y, v.z), (1.0, 2.5, -3.0));
    }

    #[test]
    fn has_exact_keys_detects_duplicates_and_missing() {
        assert!(has_exact_keys(br#"{"a":1,"b":2}"#, &["a", "b"]));
        assert!(!has_exact_keys(br#"{"a":1,"a":2}"#, &["a"]));
        assert!(!has_exact_keys(br#"{"a":1}"#, &["a", "b"]));
    }

    #[test]
    fn object_array_member_splits_elements() {
        let text = br#"{"ops":[{"id":"a"},{"id":"b"}]}"#;
        let values = object_array_member(text, "ops").unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(string_member(&values[0], "id").unwrap(), "a");
        assert_eq!(string_member(&values[1], "id").unwrap(), "b");
    }

    /// Reference values from a real compiled `printf("%.9g", (double)v)`
    /// probe (not transcribed by hand).
    #[test]
    fn format_g_matches_printf_9g() {
        let cases: &[(f32, &str)] = &[
            (0.016666667, "0.0166666675"),
            (1.0 / 60.0, "0.0166666675"),
            (0.1, "0.100000001"),
            (123456789.0, "123456792"),
            (0.0001, "9.99999975e-05"),
            (0.00001, "9.99999975e-06"),
            (100000000.0, "100000000"),
            (1000000000.0, "1e+09"),
            (-0.0, "-0"),
            (0.0, "0"),
            (3.14159, "3.14159012"),
        ];
        for (value, expected) in cases {
            assert_eq!(format_g(*value as f64, 9), *expected, "value={value}");
        }
    }

    #[test]
    fn format_fixed_matches_printf_f() {
        let cases: &[(f64, usize, &str)] = &[
            (f64::NAN, 4, "nan"),
            (-f64::NAN, 4, "nan"),
            (f64::INFINITY, 4, "inf"),
            (f64::NEG_INFINITY, 4, "-inf"),
            (-0.0, 4, "-0.0000"),
            (0.0, 4, "0.0000"),
            (3.14159, 4, "3.1416"),
            (-3.14159, 6, "-3.141590"),
        ];
        for (value, precision, expected) in cases {
            assert_eq!(format_fixed(*value, *precision), *expected, "value={value}");
        }
    }
}
