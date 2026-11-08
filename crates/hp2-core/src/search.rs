//! Port of `appStrfind` from `HarryPotter2/Unreal/Core/Src/UnMisc.cpp`.

use crate::ansi::{self, Tchar};

/// Finds the first case-insensitive occurrence whose leading character is not
/// preceded by an ASCII letter or digit. The first-character fold and boundary
/// test intentionally remain ASCII-only, matching the C++ implementation.
///
/// # Safety
/// `stream` and `find` must point to valid NUL-terminated `TCHAR` strings; the
/// length of `find` must fit the signed 32-bit `appStrlen` contract.
pub unsafe fn strfind(stream: *const Tchar, find: *const Tchar) -> *const Tchar {
    if stream.is_null() || find.is_null() {
        return std::ptr::null();
    }

    let first = unsafe { *find };
    let first_folded = if (b'a' as Tchar..=b'z' as Tchar).contains(&first) {
        first - (b'a' as Tchar - b'A' as Tchar)
    } else {
        first
    };

    let mut find_len = 0usize;
    while unsafe { *find.add(find_len) } != 0 {
        find_len += 1;
    }
    let Ok(suffix_len) = i32::try_from(find_len.saturating_sub(1)) else {
        return std::ptr::null();
    };

    let mut candidate = stream;
    let mut previous_alnum = false;
    loop {
        let c = unsafe { *candidate };
        if c == 0 {
            return std::ptr::null();
        }
        let folded = if (b'a' as Tchar..=b'z' as Tchar).contains(&c) {
            c - (b'a' as Tchar - b'A' as Tchar)
        } else {
            c
        };
        if !previous_alnum
            && folded == first_folded
            // SAFETY: the strings are NUL-terminated and the count is a prefix
            // of `find` after its first character.
            && unsafe { ansi::strnicmp(candidate.add(1), find.add(1), suffix_len) == 0 }
        {
            return candidate;
        }
        previous_alnum = (b'A' as Tchar..=b'Z' as Tchar).contains(&folded)
            || (b'0' as Tchar..=b'9' as Tchar).contains(&folded);
        candidate = unsafe { candidate.add(1) };
    }
}

/// `ParseParam`: returns true when a case-insensitive parameter occurrence is
/// preceded by `-` or `/`. Search starts after the stream's first character,
/// matching the C++ loop's `appStrfind(Start + 1, Param)` behavior.
///
/// # Safety
/// `stream` and `param` must point to valid NUL-terminated `TCHAR` strings.
pub unsafe fn parse_param(stream: *const Tchar, param: *const Tchar) -> bool {
    if stream.is_null() || param.is_null() || unsafe { *stream } == 0 {
        return false;
    }

    let mut start = stream;
    loop {
        let found = unsafe { strfind(start.add(1), param) };
        if found.is_null() {
            return false;
        }
        let lead_in = unsafe { *found.sub(1) };
        if lead_in == b'-' as Tchar || lead_in == b'/' as Tchar {
            return true;
        }
        start = found;
    }
}

#[cfg(test)]
mod param_tests {
    use super::parse_param;
    use crate::ansi::Tchar;

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    fn contains(stream: &str, param: &str) -> bool {
        let stream = wide(stream);
        let param = wide(param);
        // SAFETY: both vectors are live NUL-terminated strings.
        unsafe { parse_param(stream.as_ptr(), param.as_ptr()) }
    }

    #[test]
    fn recognizes_case_insensitive_dash_and_slash_parameters() {
        assert!(contains("game -NoRunaway /SAFE", "norunaway"));
        assert!(contains("game -NoRunaway /SAFE", "safe"));
    }

    #[test]
    fn requires_the_flag_leadin_and_keeps_prefix_matching() {
        assert!(!contains("game unsafe", "safe"));
        assert!(contains("game -SafeMode", "safe"));
    }

    #[test]
    fn rejects_empty_stream_and_empty_parameter() {
        assert!(!contains("", "safe"));
        assert!(!contains("game -safe", ""));
    }
}

#[cfg(test)]
mod tests {
    use super::strfind;
    use crate::ansi::Tchar;

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    #[test]
    fn finds_case_insensitively_only_at_non_alphanumeric_boundaries() {
        let input = wide("someFLASH flashx _fLaSh");
        let find = wide("FLASH");
        // SAFETY: both inputs are live NUL-terminated strings.
        let found = unsafe { strfind(input.as_ptr(), find.as_ptr()) };
        assert_eq!(found, unsafe { input.as_ptr().add(10) });
    }

    #[test]
    fn rejects_alphanumeric_leadin_and_finds_later_match() {
        let input = wide("xFLASH-Flash");
        let find = wide("flash");
        // SAFETY: both inputs are live NUL-terminated strings.
        let found = unsafe { strfind(input.as_ptr(), find.as_ptr()) };
        assert_eq!(found, unsafe { input.as_ptr().add(7) });
    }

    #[test]
    fn empty_or_missing_match_returns_null() {
        let input = wide("anything");
        let empty = wide("");
        let missing = wide("other");
        // SAFETY: all inputs are live NUL-terminated strings.
        assert!(unsafe { strfind(input.as_ptr(), empty.as_ptr()) }.is_null());
        assert!(unsafe { strfind(input.as_ptr(), missing.as_ptr()) }.is_null());
    }
}
