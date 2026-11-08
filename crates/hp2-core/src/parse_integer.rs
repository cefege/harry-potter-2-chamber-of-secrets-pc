//! Typed integer `Parse(Stream, Match, Value)` overloads from `UnMisc.cpp`.

use std::ffi::c_void;

use crate::ansi::{self, Hp2AnsiServices, Tchar, Threw};
use crate::search;

/// C++ integer-conversion callbacks. They preserve the existing `appAtoi` and
/// `appStrtoi` assertion and exception behavior without unwinding through Rust.
#[repr(C)]
pub struct ParseIntegerServices {
    /// Calls `appAtoi`; returns nonzero when a C++ exception was captured.
    pub atoi: unsafe extern "C" fn(*const c_void, *mut i32) -> i32,
    /// Calls `appStrtoi(Start, End, Base)`; returns nonzero on a captured exception.
    pub strtoi: unsafe extern "C" fn(*const c_void, i32, *mut i32) -> i32,
}

/// Target integer type, matching the `Parse` overload's output width and result rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum IntegerParseKind {
    Dword = 0,
    Byte = 1,
    Sbyte = 2,
    Word = 3,
    Sword = 4,
    Int = 5,
    Qword = 6,
    Sqword = 7,
}

/// Runs the source signed-INT Parse policy with the caller's real text/CRT leaves.
///
/// # Safety
/// Text is NUL-terminated; `value` is writable. Callbacks obey the native
/// `appStrlen` and `appAtoi` contracts without retaining borrowed text.
pub unsafe fn parse_int_with<E>(
    stream: *const Tchar,
    matched: *const Tchar,
    value: *mut i32,
    length: impl FnOnce(*const Tchar) -> Result<usize, E>,
    number: impl FnOnce(*const Tchar) -> Result<i32, E>,
) -> Result<bool, E> {
    if stream.is_null() || matched.is_null() || value.is_null() { return Ok(false); }
    let found = unsafe { search::strfind(stream, matched) };
    if found.is_null() { return Ok(false); }
    let parsed = number(unsafe { found.add(length(matched)?) })?;
    unsafe { *value = parsed; }
    Ok(true)
}

/// Parses one typed integer overload.
///
/// `Qword`/`Sqword` use the legacy 4096-character string parser and decimal
/// digit scan. `Dword` uses base-10 `appStrtoi` and returns true whenever the
/// key is found. The narrow types use `appAtoi` and retain the legacy `value !=
/// 0 || first character is an ASCII digit` result. `Int` uses `appAtoi` and
/// returns true whenever the key is found.
/// # Safety
/// `ansi_services` and `services` must remain valid for the call. `stream` and
/// `matched` must be NUL-terminated `TCHAR` strings; `value` must point to a
/// writable value of the selected kind.
pub unsafe fn parse_integer(
    ansi_services: &Hp2AnsiServices,
    services: &ParseIntegerServices,
    stream: *const Tchar,
    matched: *const Tchar,
    kind: IntegerParseKind,
    value: *mut c_void,
) -> Result<bool, Threw> {
    if stream.is_null() || matched.is_null() || value.is_null() {
        return Ok(false);
    }

    if kind == IntegerParseKind::Int {
        return unsafe { parse_int_with(
            stream, matched, value.cast(),
            |text| ansi::strlen(ansi_services, text).map(|size| size as usize),
            |text| {
                let mut parsed = 0;
                if (services.atoi)(text.cast(), &mut parsed) != 0 { Err(Threw) } else { Ok(parsed) }
            },
        ) };
    }

    if matches!(kind, IntegerParseKind::Qword | IntegerParseKind::Sqword) {
        let mut text = [0 as Tchar; 4096];
        // SAFETY: `text` is a positive-sized writable buffer and the inputs
        // satisfy the string parser's contract.
        if !unsafe {
            crate::text_parse::parse_string(stream, matched, text.as_mut_ptr(), text.len() as i32)
        } {
            return Ok(false);
        }
        let mut cursor = text.as_ptr();
        let negative = unsafe { *cursor == b'-' as Tchar };
        if negative {
            cursor = unsafe { cursor.add(1) };
        }
        let mut parsed = 0i64;
        while unsafe { *cursor >= b'0' as Tchar && *cursor <= b'9' as Tchar } {
            let digit = unsafe { *cursor - b'0' as Tchar } as i64;
            parsed = parsed.wrapping_mul(10).wrapping_add(digit);
            cursor = unsafe { cursor.add(1) };
        }
        if negative {
            parsed = parsed.wrapping_neg();
        }
        if kind == IntegerParseKind::Qword {
            // SAFETY: caller provides storage for the selected output type.
            unsafe { *value.cast::<u64>() = parsed as u64 };
        } else {
            // SAFETY: caller provides storage for the selected output type.
            unsafe { *value.cast::<i64>() = parsed };
        }
        return Ok(true);
    }

    // SAFETY: required by this function's pointer contract.
    let found = unsafe { search::strfind(stream, matched) };
    if found.is_null() {
        return Ok(false);
    }
    // SAFETY: `matched` is NUL-terminated and the service table outlives the call.
    let match_len = unsafe { ansi::strlen(ansi_services, matched)? };
    // SAFETY: `found` points into the live input string and `match_len` is the
    // number of wide characters in the matched key.
    let number = unsafe { found.add(match_len as usize) };

    if kind == IntegerParseKind::Dword {
        let mut parsed = 0;
        // SAFETY: the callback receives a live NUL-terminated suffix and valid output slot.
        if unsafe { (services.strtoi)(number.cast(), 10, &mut parsed) } != 0 {
            return Err(Threw);
        }
        // SAFETY: caller provides storage for the selected output type.
        unsafe { *value.cast::<u32>() = parsed as u32 };
        return Ok(true);
    }

    let mut parsed = 0;
    // SAFETY: the callback receives a live NUL-terminated suffix and valid output slot.
    if unsafe { (services.atoi)(number.cast(), &mut parsed) } != 0 {
        return Err(Threw);
    }

    let success = match kind {
        IntegerParseKind::Byte => {
            // SAFETY: caller provides storage for the selected output type.
            unsafe { *value.cast::<u8>() = parsed as u8 };
            parsed as u8 != 0 || unsafe { (*number >= b'0' as Tchar) && (*number <= b'9' as Tchar) }
        }
        IntegerParseKind::Sbyte => {
            // SAFETY: caller provides storage for the selected output type.
            unsafe { *value.cast::<i8>() = parsed as i8 };
            parsed as i8 != 0 || unsafe { (*number >= b'0' as Tchar) && (*number <= b'9' as Tchar) }
        }
        IntegerParseKind::Word => {
            // SAFETY: caller provides storage for the selected output type.
            unsafe { *value.cast::<u16>() = parsed as u16 };
            parsed as u16 != 0
                || unsafe { (*number >= b'0' as Tchar) && (*number <= b'9' as Tchar) }
        }
        IntegerParseKind::Sword => {
            // SAFETY: caller provides storage for the selected output type.
            unsafe { *value.cast::<i16>() = parsed as i16 };
            parsed as i16 != 0
                || unsafe { (*number >= b'0' as Tchar) && (*number <= b'9' as Tchar) }
        }
        IntegerParseKind::Int | IntegerParseKind::Qword | IntegerParseKind::Sqword => unreachable!(),
        IntegerParseKind::Dword => unreachable!(),
    };
    Ok(success)
}

#[cfg(test)]
mod tests {
    use super::{IntegerParseKind, ParseIntegerServices, parse_integer};
    use crate::ansi::{Hp2AnsiServices, Tchar};
    use std::ffi::c_void;

    unsafe extern "C" fn fail_assert(_: *const c_void, _: *const c_void, _: i32) -> i32 {
        0
    }

    unsafe extern "C" fn to_ansi(_: *const c_void, _: *mut *const c_void) -> i32 {
        1
    }

    unsafe extern "C" fn size_contract() -> i32 {
        0
    }

    unsafe fn parse_number(value: *const Tchar) -> i32 {
        let mut cursor = value;
        while unsafe { *cursor == b' ' as Tchar || *cursor == b'\t' as Tchar } {
            cursor = unsafe { cursor.add(1) };
        }
        let negative = unsafe { *cursor == b'-' as Tchar };
        if negative || unsafe { *cursor == b'+' as Tchar } {
            cursor = unsafe { cursor.add(1) };
        }
        let mut parsed = 0i32;
        while unsafe { *cursor >= b'0' as Tchar && *cursor <= b'9' as Tchar } {
            parsed = parsed
                .wrapping_mul(10)
                .wrapping_add(unsafe { *cursor - b'0' as Tchar });
            cursor = unsafe { cursor.add(1) };
        }
        if negative {
            parsed.wrapping_neg()
        } else {
            parsed
        }
    }

    unsafe extern "C" fn atoi(value: *const c_void, out: *mut i32) -> i32 {
        // SAFETY: the conversion service contract supplies a terminated string and output slot.
        unsafe { *out = parse_number(value.cast()) };
        0
    }

    unsafe extern "C" fn strtoi(value: *const c_void, _base: i32, out: *mut i32) -> i32 {
        // SAFETY: the conversion service contract supplies a terminated string and output slot.
        unsafe { *out = parse_number(value.cast()) };
        0
    }

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    fn parse<T>(stream: &str, matched: &str, kind: IntegerParseKind, value: &mut T) -> bool {
        let stream = wide(stream);
        let matched = wide(matched);
        let ansi_services = Hp2AnsiServices {
            fail_assert,
            to_ansi,
            size_exceeds_int_contract: size_contract,
        };
        let services = ParseIntegerServices { atoi, strtoi };
        // SAFETY: test strings and output are live and NUL-terminated/writable.
        unsafe {
            parse_integer(
                &ansi_services,
                &services,
                stream.as_ptr(),
                matched.as_ptr(),
                kind,
                (value as *mut T).cast(),
            )
            .unwrap()
        }
    }

    #[test]
    fn preserves_signed_unsigned_conversion_and_result_rules() {
        let mut byte = 0u8;
        assert!(parse(
            "count=300",
            "count=",
            IntegerParseKind::Byte,
            &mut byte
        ));
        assert_eq!(byte, 44);

        let mut signed = 0i32;
        assert!(parse(
            "count=-12",
            "count=",
            IntegerParseKind::Int,
            &mut signed
        ));
        assert_eq!(signed, -12);

        let mut dword = 0u32;
        assert!(parse(
            "count=-1",
            "count=",
            IntegerParseKind::Dword,
            &mut dword
        ));
        assert_eq!(dword, u32::MAX);
    }

    #[test]
    fn quadword_overloads_parse_quoted_decimal_and_keep_signed_bits() {
        let mut unsigned = 0u64;
        assert!(parse(
            "value=-42",
            "value=",
            IntegerParseKind::Qword,
            &mut unsigned
        ));
        assert_eq!(unsigned, u64::MAX - 41);

        let mut signed = 0i64;
        assert!(parse(
            "value=\"-9223372036854775808\"",
            "value=",
            IntegerParseKind::Sqword,
            &mut signed,
        ));
        assert_eq!(signed, i64::MIN);
    }

    #[test]
    fn narrow_overloads_distinguish_zero_digit_from_non_numeric_text() {
        let mut value = 1u16;
        assert!(parse(
            "value=0",
            "value=",
            IntegerParseKind::Word,
            &mut value
        ));
        assert_eq!(value, 0);
        assert!(!parse(
            "value=x",
            "value=",
            IntegerParseKind::Word,
            &mut value
        ));
        assert_eq!(value, 0);
    }

    #[test]
    fn missing_key_leaves_output_unchanged() {
        let mut value = 31i16;
        assert!(!parse(
            "other=7",
            "value=",
            IntegerParseKind::Sword,
            &mut value
        ));
        assert_eq!(value, 31);
    }
}
