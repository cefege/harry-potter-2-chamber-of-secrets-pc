//! Port of `ParseCommand` from `HarryPotter2/Unreal/Core/Src/UnMisc.cpp`.
//!
//! The public C++ declaration remains as an ABI wrapper for existing engine
//! callsites; the matching, token boundary, and cursor movement live here.

use crate::ansi::{self, Tchar};

#[inline]
fn is_blank(c: Tchar) -> bool {
    c == b' ' as Tchar || c == 9
}

#[inline]
fn is_ascii_alnum(c: Tchar) -> bool {
    (b'a' as Tchar..=b'z' as Tchar).contains(&c)
        || (b'A' as Tchar..=b'Z' as Tchar).contains(&c)
        || (b'0' as Tchar..=b'9' as Tchar).contains(&c)
}

/// `ParseCommand(Stream, Match)`: skips leading spaces/tabs, matches the
/// command case-insensitively, requires a non-alphanumeric boundary, then
/// consumes trailing spaces/tabs. On failure, leading spaces/tabs remain
/// consumed but the candidate command remains untouched.
///
/// # Safety
/// `stream` must point to a writable cursor; that cursor and `matched` must
/// point to valid NUL-terminated `TCHAR` strings. The match length must fit
/// the original `appStrlen` signed-int contract.
pub unsafe fn parse_command(stream: *mut *const Tchar, matched: *const Tchar) -> i32 {
    if stream.is_null() || matched.is_null() {
        return 0;
    }

    // SAFETY: required by this function's contract.
    let mut cursor = unsafe { *stream };
    if cursor.is_null() {
        return 0;
    }
    while unsafe { is_blank(*cursor) } {
        cursor = unsafe { cursor.add(1) };
    }
    // C++ advances `*Stream` over leading blanks even when matching fails.
    unsafe { *stream = cursor };

    let mut match_len = 0usize;
    while unsafe { *matched.add(match_len) } != 0 {
        match_len += 1;
    }
    let Ok(match_len_i32) = i32::try_from(match_len) else {
        return 0;
    };

    // `appStrnicmp` is already ported to the same host `wcsncasecmp` call.
    // SAFETY: both pointers are NUL-terminated and `match_len` is in range.
    if unsafe { ansi::strnicmp(cursor, matched, match_len_i32) } != 0 {
        return 0;
    }

    let mut after_match = unsafe { cursor.add(match_len) };
    if unsafe { is_ascii_alnum(*after_match) } {
        return 0;
    }
    while unsafe { is_blank(*after_match) } {
        after_match = unsafe { after_match.add(1) };
    }
    unsafe { *stream = after_match };
    1
}

#[cfg(test)]
mod tests {
    use super::parse_command;
    use crate::ansi::Tchar;

    fn text(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).collect()
    }

    fn wide(value: &str) -> Vec<Tchar> {
        text(value).into_iter().chain([0]).collect()
    }

    unsafe fn text_at<'a>(cursor: *const Tchar) -> &'a [Tchar] {
        let mut len = 0;
        while unsafe { *cursor.add(len) } != 0 {
            len += 1;
        }
        unsafe { std::slice::from_raw_parts(cursor, len) }
    }

    #[test]
    fn command_match_consumes_leading_and_trailing_blanks_case_insensitively() {
        let input = wide("\t  fLaSh  next");
        let matched = wide("FLASH");
        let mut cursor = input.as_ptr();
        // SAFETY: both vectors are NUL-terminated and live for this call.
        assert_eq!(unsafe { parse_command(&mut cursor, matched.as_ptr()) }, 1);
        // SAFETY: cursor remains within the input allocation.
        assert_eq!(unsafe { text_at(cursor) }, text("next").as_slice());
    }

    #[test]
    fn failed_match_consumes_only_leading_blanks_and_preserves_alphanumeric_prefix() {
        let input = wide(" \t FlushNow");
        let matched = wide("Flush");
        let mut cursor = input.as_ptr();
        // SAFETY: both vectors are NUL-terminated and live for this call.
        assert_eq!(unsafe { parse_command(&mut cursor, matched.as_ptr()) }, 0);
        // SAFETY: cursor points inside the input allocation at `FlushNow`.
        assert_eq!(unsafe { text_at(cursor) }, text("FlushNow").as_slice());
    }

    #[test]
    fn punctuation_is_a_command_boundary_but_newline_is_not_skipped() {
        let input = wide("ITEM_name");
        let matched = wide("item");
        let mut cursor = input.as_ptr();
        // SAFETY: both vectors are NUL-terminated and live for this call.
        assert_eq!(unsafe { parse_command(&mut cursor, matched.as_ptr()) }, 1);
        // SAFETY: cursor remains within the input allocation.
        assert_eq!(unsafe { text_at(cursor) }, text("_name").as_slice());

        let input = wide("\nITEM");
        let mut cursor = input.as_ptr();
        // SAFETY: both vectors are NUL-terminated and live for this call.
        assert_eq!(unsafe { parse_command(&mut cursor, matched.as_ptr()) }, 0);
        // SAFETY: no leading spaces/tabs were consumed.
        assert_eq!(unsafe { text_at(cursor) }, text("\nITEM").as_slice());
    }
}
