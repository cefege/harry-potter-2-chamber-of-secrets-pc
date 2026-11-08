//! Ports of `ParseNext`, `ParseToken`, and `ParseLine` from `UnMisc.cpp`.

use crate::ansi::Tchar;

#[inline]
fn is_parse_next_blank(c: Tchar) -> bool {
    c == b' ' as Tchar || c == 9 || c == 13 || c == 10
}

#[inline]
fn is_token_blank(c: Tchar) -> bool {
    c == b' ' as Tchar || c == 9
}

/// `ParseNext`: skips spaces, tabs, CR/LF, and semicolon comments.
///
/// # Safety
/// `stream` must point to a writable cursor into a NUL-terminated `TCHAR` string.
pub unsafe fn parse_next(stream: *mut *const Tchar) {
    if stream.is_null() {
        return;
    }
    let mut cursor = unsafe { *stream };
    if cursor.is_null() {
        return;
    }

    loop {
        while unsafe { is_parse_next_blank(*cursor) } {
            cursor = unsafe { cursor.add(1) };
        }
        if unsafe { *cursor } != b';' as Tchar {
            break;
        }
        while unsafe { *cursor != 0 && *cursor != 10 && *cursor != 13 } {
            cursor = unsafe { cursor.add(1) };
        }
    }
    unsafe { *stream = cursor };
}

/// `ParseToken`: reads a quoted or whitespace-delimited token and advances the
/// cursor. Unquoted input is consumed even when the output buffer truncates it;
/// quoted input stops at the output limit, matching the original implementation.
///
/// # Safety
/// `stream` must point to a writable cursor into a NUL-terminated `TCHAR` string;
/// `result` must have `max_len` writable characters and `max_len` must be positive.
pub unsafe fn parse_token(
    stream: *mut *const Tchar,
    result: *mut Tchar,
    max_len: i32,
    use_escape: bool,
) -> bool {
    if stream.is_null() || result.is_null() {
        return false;
    }
    let mut cursor = unsafe { *stream };
    if cursor.is_null() {
        return false;
    }

    while unsafe { is_token_blank(*cursor) } {
        cursor = unsafe { cursor.add(1) };
    }

    let capacity = max_len as usize;
    let mut len = 0usize;
    if unsafe { *cursor == b'"' as Tchar } {
        cursor = unsafe { cursor.add(1) };
        while unsafe { *cursor != 0 && *cursor != b'"' as Tchar } && len + 1 < capacity {
            let mut c = unsafe { *cursor };
            cursor = unsafe { cursor.add(1) };
            if c == b'\\' as Tchar && use_escape {
                c = unsafe { *cursor };
                cursor = unsafe { cursor.add(1) };
                if c == 0 {
                    break;
                }
            }
            if len + 1 < capacity {
                unsafe { *result.add(len) = c };
                len += 1;
            }
        }
        if unsafe { *cursor == b'"' as Tchar } {
            cursor = unsafe { cursor.add(1) };
        }
    } else {
        while unsafe { *cursor != 0 && !is_token_blank(*cursor) } {
            if len + 1 < capacity {
                unsafe { *result.add(len) = *cursor };
                len += 1;
            }
            cursor = unsafe { cursor.add(1) };
        }
    }

    unsafe {
        *result.add(len) = 0;
        *stream = cursor;
    }
    len != 0
}

/// `ParseLine`: returns text through CR/LF, respecting quotes, comments,
/// command separators, output truncation, and the `Exact` mode.
///
/// # Safety
/// `stream` must point to a writable cursor into a NUL-terminated `TCHAR` string;
/// `result` must have `max_len` writable characters and `max_len` must be positive.
pub unsafe fn parse_line(
    stream: *mut *const Tchar,
    result: *mut Tchar,
    max_len: i32,
    exact: bool,
) -> bool {
    if stream.is_null() || result.is_null() {
        return false;
    }
    let mut cursor = unsafe { *stream };
    if cursor.is_null() {
        return false;
    }

    unsafe { *result = 0 };
    let mut remaining = max_len;
    let mut len = 0usize;
    let mut got_stream = false;
    let mut is_quoted = false;
    let mut ignore = false;

    while unsafe { *cursor != 0 && *cursor != 10 && *cursor != 13 } && {
        remaining -= 1;
        remaining > 0
    } {
        if !is_quoted
            && !exact
            && unsafe { *cursor == b'/' as Tchar && *cursor.add(1) == b'/' as Tchar }
        {
            ignore = true;
        }
        if !is_quoted && !exact && unsafe { *cursor == b'|' as Tchar } {
            break;
        }

        is_quoted ^= unsafe { *cursor == b'"' as Tchar };
        got_stream = true;
        if !ignore {
            unsafe { *result.add(len) = *cursor };
            len += 1;
        }
        cursor = unsafe { cursor.add(1) };
    }

    if exact {
        if unsafe { *cursor == 13 } {
            cursor = unsafe { cursor.add(1) };
        }
        if unsafe { *cursor == 10 } {
            cursor = unsafe { cursor.add(1) };
        }
    } else {
        while unsafe { *cursor == 10 || *cursor == 13 || *cursor == b'|' as Tchar } {
            cursor = unsafe { cursor.add(1) };
        }
    }

    unsafe {
        *result.add(len) = 0;
        *stream = cursor;
    }
    (unsafe { *cursor != 0 }) || got_stream
}

#[cfg(test)]
mod tests {
    use super::{parse_line, parse_next, parse_token};
    use crate::ansi::Tchar;

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    unsafe fn suffix(cursor: *const Tchar) -> Vec<Tchar> {
        let mut result = Vec::new();
        let mut p = cursor;
        unsafe {
            while *p != 0 {
                result.push(*p);
                p = p.add(1);
            }
        }
        result
    }

    unsafe fn contents(buffer: *const Tchar) -> Vec<Tchar> {
        unsafe { suffix(buffer) }
    }

    #[test]
    fn parse_next_skips_whitespace_and_repeated_comments() {
        let input = wide(" \t;one\r\n;two\n TOKEN");
        let mut cursor = input.as_ptr();
        // SAFETY: `cursor` points into a live NUL-terminated input vector.
        unsafe { parse_next(&mut cursor) };
        assert_eq!(unsafe { suffix(cursor) }, wide("TOKEN")[..5]);
    }

    #[test]
    fn parse_token_handles_escaped_quotes_and_unquoted_truncation() {
        let input = wide("  \"a\\\"b\" tail");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 16];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_token(&mut cursor, output.as_mut_ptr(), 16, true) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("a\"b")[..3]);
        assert_eq!(unsafe { suffix(cursor) }, wide(" tail")[..5]);

        let input = wide("abcdef next");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 5];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_token(&mut cursor, output.as_mut_ptr(), 5, false) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("abcd")[..4]);
        assert_eq!(unsafe { suffix(cursor) }, wide(" next")[..5]);
    }

    #[test]
    fn parse_token_quoted_truncation_leaves_unread_tail() {
        let input = wide("\"abcdef\" next");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 5];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_token(&mut cursor, output.as_mut_ptr(), 5, false) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("abcd")[..4]);
        assert_eq!(unsafe { suffix(cursor) }, wide("ef\" next")[..8]);
    }

    #[test]
    fn parse_token_without_escape_stops_at_the_first_quote() {
        let input = wide("\"a\\\"b\" tail");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 16];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_token(&mut cursor, output.as_mut_ptr(), 16, false) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("a\\")[..2]);
        assert_eq!(unsafe { suffix(cursor) }, wide("b\" tail")[..7]);
    }

    #[test]
    fn parse_line_skips_comments_and_splits_commands() {
        let input = wide("one//hide\r\nnext|last\n");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 32];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_line(&mut cursor, output.as_mut_ptr(), 32, false) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("one")[..3]);
        assert_eq!(unsafe { suffix(cursor) }, wide("next|last\n")[..10]);

        assert!(unsafe { parse_line(&mut cursor, output.as_mut_ptr(), 32, false) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("next")[..4]);
        assert_eq!(unsafe { suffix(cursor) }, wide("last\n")[..5]);
    }

    #[test]
    fn parse_line_exact_preserves_comments_and_pipes_but_eats_one_crlf() {
        let input = wide("a//b|c\r\nnext");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 32];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_line(&mut cursor, output.as_mut_ptr(), 32, true) });
        assert_eq!(unsafe { contents(output.as_ptr()) }, wide("a//b|c")[..6]);
        assert_eq!(unsafe { suffix(cursor) }, wide("next")[..4]);
    }

    #[test]
    fn parse_line_keeps_slashes_and_pipes_inside_quotes() {
        let input = wide("\"x//y|z\"//hide\nnext");
        let mut cursor = input.as_ptr();
        let mut output = vec![0; 32];
        // SAFETY: input and output have valid NUL-terminated/writable storage.
        assert!(unsafe { parse_line(&mut cursor, output.as_mut_ptr(), 32, false) });
        assert_eq!(
            unsafe { contents(output.as_ptr()) },
            wide("\"x//y|z\"")[..8]
        );
        assert_eq!(unsafe { suffix(cursor) }, wide("next")[..4]);
    }
}
