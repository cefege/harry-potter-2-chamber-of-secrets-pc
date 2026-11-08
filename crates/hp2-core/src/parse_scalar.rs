//! Float and boolean `Parse` overloads from `UnMisc.cpp`.

use crate::ansi::{self, Hp2AnsiServices, Tchar, Threw};
use crate::search;

/// Parses a floating-point value following a case-insensitive key.
///
/// # Safety
/// `services` must remain valid; `stream` and `matched` must be NUL-terminated
/// `TCHAR` strings; `value` must point to a writable `f32`.
pub unsafe fn parse_float(
    services: &Hp2AnsiServices,
    stream: *const Tchar,
    matched: *const Tchar,
    value: *mut f32,
) -> Result<bool, Threw> {
    let length = |text| unsafe { ansi::strlen(services, text).map(|length| length as usize) };
    let number = |text| unsafe { ansi::atof(services, text) };
    unsafe { parse_float_with(stream, matched, value, length, number) }
}

/// Runs the shared float-key policy with the caller's string and numeric SDKs.
///
/// # Safety
/// Strings must be NUL-terminated and `value` writable when non-null. `length`
/// must return the matched string's length; `number` receives a live suffix.
pub unsafe fn parse_float_with<E>(
    stream: *const Tchar,
    matched: *const Tchar,
    value: *mut f32,
    length: impl FnOnce(*const Tchar) -> Result<usize, E>,
    number: impl FnOnce(*const Tchar) -> Result<f32, E>,
) -> Result<bool, E> {
    if stream.is_null() || matched.is_null() || value.is_null() {
        return Ok(false);
    }
    let found = unsafe { search::strfind(stream, matched) };
    if found.is_null() {
        return Ok(false);
    }
    let match_len = length(matched)?;
    let suffix = unsafe { found.add(match_len) };
    let parsed = number(suffix)?;
    unsafe { *value = parsed };
    Ok(true)
}

/// Parses a localized boolean value following a key. Recognized true spellings
/// are `On`, `True`, `GTrue`, and `1`; every other found value is false.
///
/// # Safety
/// `stream`, `matched`, and `localized_true` must be NUL-terminated `TCHAR`
/// strings; `value` must point to a writable `i32`.
pub unsafe fn parse_bool(
    stream: *const Tchar,
    matched: *const Tchar,
    localized_true: *const Tchar,
    value: *mut i32,
) -> bool {
    if stream.is_null() || matched.is_null() || localized_true.is_null() || value.is_null() {
        return false;
    }
    let mut text = [0 as Tchar; 16];
    // SAFETY: the inputs are terminated, `text` is writable, and the capacity is positive.
    if !unsafe { crate::text_parse::parse_string(stream, matched, text.as_mut_ptr(), 16) } {
        return false;
    }
    let on = [b'O' as Tchar, b'n' as Tchar, 0];
    let truth = [
        b'T' as Tchar,
        b'r' as Tchar,
        b'u' as Tchar,
        b'e' as Tchar,
        0,
    ];
    let one = [b'1' as Tchar, 0];
    // SAFETY: all comparison strings are terminated and `text` is terminated by parse_string.
    let parsed = unsafe {
        ansi::stricmp(text.as_ptr(), on.as_ptr()) == 0
            || ansi::stricmp(text.as_ptr(), truth.as_ptr()) == 0
            || ansi::stricmp(text.as_ptr(), localized_true) == 0
            || ansi::stricmp(text.as_ptr(), one.as_ptr()) == 0
    };
    // SAFETY: caller provides writable result storage.
    unsafe { *value = i32::from(parsed) };
    true
}

#[cfg(test)]
mod tests {
    use super::{parse_bool, parse_float_with};
    use crate::ansi::Tchar;

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    fn parse(stream: &str, matched: &str, localized_true: &str, initial: i32) -> (bool, i32) {
        let stream = wide(stream);
        let matched = wide(matched);
        let localized_true = wide(localized_true);
        let mut value = initial;
        // SAFETY: all strings are NUL-terminated and `value` is writable.
        let found = unsafe {
            parse_bool(
                stream.as_ptr(),
                matched.as_ptr(),
                localized_true.as_ptr(),
                &mut value,
            )
        };
        (found, value)
    }

    #[test]
    fn recognizes_standard_and_localized_true_values_case_insensitively() {
        for text in ["On", "tRuE", "1", "Oui"] {
            let (found, value) = parse(&format!("flag={text}"), "flag=", "Oui", 0);
            assert!(found);
            assert_eq!(value, 1, "value {text:?} should be true");
        }
    }

    #[test]
    fn found_false_values_clear_output_but_missing_key_preserves_it() {
        let (found, value) = parse("flag=off", "flag=", "Oui", 1);
        assert!(found);
        assert_eq!(value, 0);
        let (found, value) = parse("other=On", "flag=", "Oui", 1);
        assert!(!found);
        assert_eq!(value, 1);
    }

    #[test]
    fn float_key_boundaries_and_invalid_values_follow_source_parse() {
        for (text, expected) in [
            ("XSPEED=2 speed=3.5", (true, 3.5)),
            ("XSPEED=2", (false, 9.0)),
            ("SPEED=invalid", (true, 0.0)),
        ] {
            let stream = wide(text);
            let key = wide("SPEED=");
            let mut value = 9.0;
            let length = |_| Ok::<_, std::convert::Infallible>(key.len() - 1);
            let number = |suffix: *const Tchar| {
                let mut bytes = Vec::new();
                while unsafe { *suffix.add(bytes.len()) } != 0 {
                    bytes.push(unsafe { *suffix.add(bytes.len()) } as u8);
                }
                bytes.push(0);
                Ok(unsafe { libc::atof(bytes.as_ptr().cast()) } as f32)
            };
            // SAFETY: both vectors are terminated and value is writable;
            // conversion uses a private byte buffer, not the global ANSI ring.
            let found = unsafe {
                parse_float_with(stream.as_ptr(), key.as_ptr(), &mut value, length, number)
            }.unwrap();
            assert_eq!((found, value), expected, "{text}");
        }
    }
}
