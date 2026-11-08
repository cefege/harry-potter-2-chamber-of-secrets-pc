//! GUID `Parse(Stream, Match, FGuid&)` implementation from `UnMisc.cpp`.

use crate::ansi::{self, Hp2AnsiServices, Tchar, Threw};
use crate::parse_integer::ParseIntegerServices;

/// Parses a 32-character hexadecimal GUID into four DWORD fields.
///
/// The legacy parser returns success for any found value; values whose length
/// is not exactly 32 produce an all-zero GUID. Each eight-character segment
/// uses the existing C++ `appStrtoi` callback, preserving its conversion rules.
///
/// # Safety
/// `ansi_services` and `integer_services` must remain valid. `stream` and
/// `matched` must be NUL-terminated `TCHAR` strings. Every pointer in `guid`
/// must point to a writable `u32`.
pub unsafe fn parse_guid(
    ansi_services: &Hp2AnsiServices,
    integer_services: &ParseIntegerServices,
    stream: *const Tchar,
    matched: *const Tchar,
    guid: [*mut u32; 4],
) -> Result<bool, Threw> {
    if stream.is_null() || matched.is_null() || guid.iter().any(|field| field.is_null()) {
        return Ok(false);
    }

    let mut text = [0 as Tchar; 256];
    // SAFETY: the inputs are terminated and the local buffer has 256 writable slots.
    if !unsafe { crate::text_parse::parse_string(stream, matched, text.as_mut_ptr(), 256) } {
        return Ok(false);
    }

    // The C++ parser clears the output as soon as a key is found, even when
    // the value has an unsupported length.
    for field in guid {
        // SAFETY: each pointer denotes one writable DWORD output field.
        unsafe { *field = 0 };
    }
    // SAFETY: `text` is NUL-terminated and the callback table outlives this call.
    if unsafe { ansi::strlen(ansi_services, text.as_ptr())? } != 32 {
        return Ok(true);
    }

    // Preserve the original conversion order (D, C, B, A) and delimit each
    // eight-character field before calling appStrtoi.
    for (offset, field) in [(24usize, 3usize), (16, 2), (8, 1), (0, 0)] {
        text[offset + 8] = 0;
        let mut parsed = 0i32;
        // SAFETY: the selected suffix is terminated, and parsed is writable.
        if unsafe { (integer_services.strtoi)(text.as_ptr().add(offset).cast(), 16, &mut parsed) }
            != 0
        {
            return Err(Threw);
        }
        // SAFETY: each pointer denotes one writable DWORD output field.
        unsafe { *guid[field] = parsed as u32 };
    }
    Ok(true)
}
