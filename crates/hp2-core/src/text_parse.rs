//! Port of `Parse(Stream, Match, TCHAR*, MaxLen)` from `UnMisc.cpp`.

use crate::ansi::{self, Tchar};
use crate::search;

/// Parses a value following a case-insensitive key. Quoted values keep spaces
/// until the next quote; unquoted values stop at the first space, CR, LF, tab,
/// or comma. A missing key leaves `value` unchanged.
///
/// # Safety
/// `stream` and `matched` must be NUL-terminated `TCHAR` strings, `value` must
/// point to a writable buffer of at least `max_len` characters, `max_len` must
/// be positive, and the length of `matched` must fit the signed 32-bit
/// `appStrlen` contract.
pub unsafe fn parse_string(
    stream: *const Tchar,
    matched: *const Tchar,
    value: *mut Tchar,
    max_len: i32,
) -> bool {
    if stream.is_null() || matched.is_null() || value.is_null() {
        return false;
    }

    // SAFETY: required by this function's pointer contract.
    let found = unsafe { search::strfind(stream, matched) };
    if found.is_null() {
        return false;
    }

    let mut match_len = 0usize;
    while unsafe { *matched.add(match_len) } != 0 {
        match_len += 1;
    }
    let mut start = unsafe { found.add(match_len) };
    let quoted = unsafe { *start == b'"' as Tchar };
    if quoted {
        start = unsafe { start.add(1) };
    }

    // SAFETY: `value` has `max_len` writable elements, `start` is NUL-terminated,
    // and the function contract requires a positive `max_len`.
    unsafe { ansi::strncpy(value, start, max_len) };

    if quoted {
        let quote = [b'"' as Tchar, 0];
        // SAFETY: `value` is NUL-terminated by `strncpy` and `quote` is terminated.
        let end = unsafe { ansi::strstr(value, quote.as_ptr()) };
        if !end.is_null() {
            unsafe { *end = 0 };
        }
    } else {
        for delimiter in [
            b' ' as Tchar,
            b'\r' as Tchar,
            b'\n' as Tchar,
            b'\t' as Tchar,
            b',' as Tchar,
        ] {
            let needle = [delimiter, 0];
            // SAFETY: `value` is NUL-terminated by `strncpy`; `needle` is terminated.
            let end = unsafe { ansi::strstr(value, needle.as_ptr()) };
            if !end.is_null() {
                unsafe { *end = 0 };
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::parse_string;
    use crate::ansi::Tchar;

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    fn parse(stream: &str, matched: &str, max_len: usize) -> (bool, Vec<Tchar>) {
        let stream = wide(stream);
        let matched = wide(matched);
        let mut value = vec![0; max_len];
        // SAFETY: inputs are terminated and `value` is a positive-sized writable buffer.
        let parsed = unsafe {
            parse_string(
                stream.as_ptr(),
                matched.as_ptr(),
                value.as_mut_ptr(),
                max_len as i32,
            )
        };
        (parsed, value)
    }

    fn contents(value: &[Tchar]) -> Vec<Tchar> {
        let end = value.iter().position(|&c| c == 0).unwrap_or(value.len());
        value[..end].to_vec()
    }

    #[test]
    fn quoted_values_preserve_spaces_and_stop_at_the_closing_quote() {
        let (parsed, value) = parse("name=\"Harry Potter\" tail", "NAME=", 32);
        assert!(parsed);
        assert_eq!(contents(&value), wide("Harry Potter")[..12]);
    }

    #[test]
    fn unquoted_values_stop_at_each_legacy_delimiter() {
        for suffix in [" rest", ",rest", "\rrest", "\nrest", "\trest"] {
            let stream = format!("key=value{suffix}");
            let (parsed, value) = parse(&stream, "key=", 32);
            assert!(parsed);
            assert_eq!(contents(&value), wide("value")[..5]);
        }
    }

    #[test]
    fn truncates_to_buffer_capacity_and_preserves_output_on_missing_key() {
        let (parsed, value) = parse("key=abcdef", "key=", 5);
        assert!(parsed);
        assert_eq!(contents(&value), wide("abcd")[..4]);

        let stream = wide("other=value");
        let matched = wide("key=");
        let mut value = wide("keep");
        // SAFETY: inputs are terminated and the output buffer has five elements.
        let parsed =
            unsafe { parse_string(stream.as_ptr(), matched.as_ptr(), value.as_mut_ptr(), 5) };
        assert!(!parsed);
        assert_eq!(contents(&value), wide("keep")[..4]);
    }
}
