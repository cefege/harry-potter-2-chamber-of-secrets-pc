//! Portable path-string helpers from `HarryPotter2/Unreal/Core/Src/UnMisc.cpp`.

use crate::ansi::Tchar;

/// `appFExt`: returns the suffix after the first colon, last forward slash, and
/// last dot, in that order. With no dot, this is the resulting basename.
///
/// # Safety
/// `name` must point to a readable NUL-terminated TCHAR string.
pub unsafe fn app_fext(name: *const Tchar) -> *const Tchar {
    let mut start = name;
    let mut cursor = name;
    while unsafe { *cursor } != 0 {
        if unsafe { *cursor } == b':' as Tchar {
            start = unsafe { cursor.add(1) };
            break;
        }
        cursor = unsafe { cursor.add(1) };
    }

    cursor = start;
    let mut after_slash = start;
    while unsafe { *cursor } != 0 {
        if unsafe { *cursor } == b'/' as Tchar {
            after_slash = unsafe { cursor.add(1) };
        }
        cursor = unsafe { cursor.add(1) };
    }

    cursor = after_slash;
    let mut after_dot = after_slash;
    while unsafe { *cursor } != 0 {
        if unsafe { *cursor } == b'.' as Tchar {
            after_dot = unsafe { cursor.add(1) };
        }
        cursor = unsafe { cursor.add(1) };
    }
    after_dot
}

/// `appPathLeaf`: return the component after the last slash or backslash.
///
/// # Safety
/// `path` must point to a readable NUL-terminated TCHAR string.
pub unsafe fn app_path_leaf(path: *const Tchar) -> *const Tchar {
    let mut leaf = path;
    let mut cursor = path;
    while unsafe { *cursor } != 0 {
        let ch = unsafe { *cursor };
        if ch == b'/' as Tchar || ch == b'\\' as Tchar {
            leaf = unsafe { cursor.add(1) };
        }
        cursor = unsafe { cursor.add(1) };
    }
    leaf
}

/// Convert a POSIX `TCHAR` filename to normalized UTF-8 without allocating.
/// Backslashes become slashes, repeated slashes collapse, and invalid Unicode
/// scalar values become U+FFFD, matching `appToUtf8NativePath`.
///
/// # Safety
/// `input` must be a readable NUL-terminated 32-bit TCHAR string. `output`
/// must be writable for `capacity` bytes; it may be null only when capacity is
/// zero. The caller must provide at least four bytes per input code unit.
pub unsafe fn native_path_to_utf8(
    input: *const Tchar,
    output: *mut u8,
    capacity: usize,
) -> Option<usize> {
    if input.is_null() || (output.is_null() && capacity != 0) {
        return None;
    }

    let mut cursor = input;
    let mut written = 0usize;
    let mut previous_slash = false;
    loop {
        // SAFETY: `input` is a readable NUL-terminated TCHAR string.
        let value = unsafe { *cursor };
        if value == 0 {
            return Some(written);
        }
        cursor = unsafe { cursor.add(1) };

        let signed_value = value as i64;
        let mut codepoint = if (0..=0x10ffff).contains(&signed_value)
            && !(0xd800..=0xdfff).contains(&signed_value)
        {
            signed_value as u32
        } else {
            0xfffd
        };

        if codepoint == b'\\' as u32 {
            codepoint = b'/' as u32;
        }
        let is_slash = codepoint == b'/' as u32;
        if is_slash && previous_slash {
            continue;
        }
        previous_slash = is_slash;

        let mut encoded = [0u8; 4];
        let encoded_len = if codepoint <= 0x7f {
            encoded[0] = codepoint as u8;
            1
        } else if codepoint <= 0x7ff {
            encoded[0] = (0xc0 | (codepoint >> 6)) as u8;
            encoded[1] = (0x80 | (codepoint & 0x3f)) as u8;
            2
        } else if codepoint <= 0xffff {
            encoded[0] = (0xe0 | (codepoint >> 12)) as u8;
            encoded[1] = (0x80 | ((codepoint >> 6) & 0x3f)) as u8;
            encoded[2] = (0x80 | (codepoint & 0x3f)) as u8;
            3
        } else {
            encoded[0] = (0xf0 | (codepoint >> 18)) as u8;
            encoded[1] = (0x80 | ((codepoint >> 12) & 0x3f)) as u8;
            encoded[2] = (0x80 | ((codepoint >> 6) & 0x3f)) as u8;
            encoded[3] = (0x80 | (codepoint & 0x3f)) as u8;
            4
        };

        let Some(end) = written.checked_add(encoded_len) else {
            return None;
        };
        if end > capacity {
            return None;
        }
        // SAFETY: capacity was checked and `output` is writable for that range.
        unsafe {
            core::ptr::copy_nonoverlapping(encoded.as_ptr(), output.add(written), encoded_len)
        };
        written = end;
    }
}

#[cfg(test)]
mod tests {
    use super::{app_fext, app_path_leaf, native_path_to_utf8};
    use crate::ansi::Tchar;

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    unsafe fn text(mut pointer: *const Tchar) -> String {
        let mut result = String::new();
        while unsafe { *pointer } != 0 {
            result.push(char::from_u32(unsafe { *pointer } as u32).unwrap());
            pointer = unsafe { pointer.add(1) };
        }
        result
    }

    #[test]
    fn preserves_legacy_extension_and_basename_selection() {
        for (input, expected) in [
            ("Sounds/Music/song.ogg", "ogg"),
            ("ogg:Music/a.b.ogg", "ogg"),
            ("noext", "noext"),
            ("dir.x/file", "file"),
            ("a.b.c.ogg", "ogg"),
            ("C:\\x.wav", "wav"),
            ("", ""),
            ("/", ""),
            (".", ""),
            ("x:y/z.w:v", "w:v"),
            ("a:b:c", "b:c"),
            ("trailing.", ""),
            ("a/b/c", "c"),
        ] {
            let input = wide(input);
            // SAFETY: `input` is NUL-terminated and lives for the conversion.
            let actual = unsafe { text(app_fext(input.as_ptr())) };
            assert_eq!(actual, expected, "appFExt({input:?})");
        }
    }

    #[test]
    fn path_leaf_recognizes_both_separator_styles_without_copying() {
        for (input, expected) in [
            ("", ""),
            ("Entry.unr", "Entry.unr"),
            ("Maps/Entry.unr", "Entry.unr"),
            ("..\\Maps/Entry.unr", "Entry.unr"),
            ("a//b\\\\c", "c"),
            ("/", ""),
            ("trailing\\", ""),
            ("http://host/path", "path"),
        ] {
            let input = wide(input);
            // SAFETY: `input` is NUL-terminated and lives for the conversion.
            let result = unsafe { app_path_leaf(input.as_ptr()) };
            let actual = unsafe { text(result) };
            assert_eq!(actual, expected, "appPathLeaf({input:?})");
            assert_eq!(
                result,
                unsafe { input.as_ptr().add(input.len() - expected.len() - 1) },
                "appPathLeaf must return a pointer into the original string"
            );
        }
    }

    #[test]
    fn native_path_to_utf8_normalizes_and_replaces_invalid_scalars() {
        fn convert(input: &[Tchar]) -> Vec<u8> {
            // SAFETY: `input` is NUL-terminated; the buffer holds four bytes
            // per input code unit, the bound the conversion contract requires.
            let mut output = vec![0u8; input.len() * 4];
            let written =
                unsafe { native_path_to_utf8(input.as_ptr(), output.as_mut_ptr(), output.len()) };
            let written = written.expect("native path conversion failed");
            output.truncate(written);
            output
        }

        // `{'A','\\','\\','B','/','/','C'}`: backslashes become slashes and
        // repeated separators collapse, matching `appToUtf8NativePath`.
        let separators: Vec<Tchar> =
            [b'A', b'\\', b'\\', b'B', b'/', b'/', b'C', 0].map(|byte| byte as Tchar).to_vec();
        assert_eq!(convert(&separators), b"A/B/C");

        // `{0x1f642, '/', 0x110000}`: U+1F642 becomes its UTF-8 encoding, the
        // slash stays, and the invalid scalar 0x110000 becomes U+FFFD.
        let unicode: Vec<Tchar> = [0x1f642 as Tchar, b'/' as Tchar, 0x110000 as Tchar, 0].to_vec();
        assert_eq!(convert(&unicode), b"\xf0\x9f\x99\x82/\xef\xbf\xbd");

        // Empty path: zero bytes written and the output buffer left untouched.
        let empty: [Tchar; 1] = [0];
        let mut output = [0xccu8; 4];
        let written =
            unsafe { native_path_to_utf8(empty.as_ptr(), output.as_mut_ptr(), output.len()) };
        assert_eq!(written, Some(0));
        assert_eq!(output, [0xccu8; 4]);
    }
}
