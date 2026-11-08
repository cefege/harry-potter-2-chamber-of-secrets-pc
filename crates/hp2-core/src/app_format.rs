//! `appFormat` placeholder substitution logic from `UnMisc.cpp`.

use std::ffi::c_void;

use crate::ansi::Tchar;
use crate::ansi::Threw;

/// C++ callbacks for FString-keyed map lookup and final FString assignment.
#[repr(C)]
pub struct AppFormatServices {
    /// Finds a map value by case-insensitive FString key; null means absent.
    pub find_value: unsafe extern "C" fn(*const c_void, *const c_void, *mut *const c_void) -> i32,
    /// Assigns a NUL-terminated TCHAR result to an FString output.
    pub assign_result: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
}

/// Substitutes `%key%` placeholders, matching the legacy alternating parser.
///
/// A missing closed placeholder remains `%key%`; a missing final unclosed
/// placeholder remains `%key`. Replacement values are copied verbatim and are
/// not parsed recursively. Key matching stays in `TMultiMap<FString, FString>`
/// so its case-insensitive FString equality remains authoritative.
///
/// # Safety
/// `services` must remain valid; `source` must be a NUL-terminated `TCHAR`
/// string; `map` must be a valid C++ `TMultiMap<FString, FString>`; `result`
/// must point to a writable C++ FString.
pub unsafe fn app_format(
    services: &AppFormatServices,
    source: *const Tchar,
    map: *const c_void,
    result: *mut c_void,
) -> Result<(), Threw> {
    if source.is_null() || map.is_null() || result.is_null() {
        return Err(Threw);
    }

    let mut source_len = 0usize;
    while unsafe { *source.add(source_len) } != 0 {
        source_len += 1;
    }
    let mut output = Vec::with_capacity(source_len.saturating_add(1));
    let mut cursor = 0usize;
    let mut placeholder = false;

    loop {
        let mut end = cursor;
        while end < source_len && unsafe { *source.add(end) } != b'%' as Tchar {
            end += 1;
        }

        if !placeholder {
            for index in cursor..end {
                output.push(unsafe { *source.add(index) });
            }
        } else {
            let mut key = Vec::with_capacity(end - cursor + 1);
            for index in cursor..end {
                key.push(unsafe { *source.add(index) });
            }
            key.push(0);

            let mut value = std::ptr::null();
            // SAFETY: `key` is NUL-terminated and both callback outputs are valid.
            if unsafe { (services.find_value)(map, key.as_ptr().cast(), &mut value) } != 0 {
                return Err(Threw);
            }
            if value.is_null() {
                output.push(b'%' as Tchar);
                output.extend_from_slice(&key[..key.len() - 1]);
                if end < source_len {
                    output.push(b'%' as Tchar);
                }
            } else {
                let mut value = value.cast::<Tchar>();
                while unsafe { *value } != 0 {
                    output.push(unsafe { *value });
                    value = unsafe { value.add(1) };
                }
            }
        }

        if end == source_len {
            break;
        }
        cursor = end + 1;
        placeholder = !placeholder;
    }

    output.push(0);
    // SAFETY: the output vector is NUL-terminated and both pointers meet callback contracts.
    if unsafe { (services.assign_result)(result, output.as_ptr().cast()) } != 0 {
        return Err(Threw);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AppFormatServices, app_format};
    use crate::ansi::Tchar;
    use std::ffi::c_void;

    struct State {
        entries: Vec<(Vec<Tchar>, Vec<Tchar>)>,
        output: Vec<Tchar>,
    }

    fn wide(text: &str) -> Vec<Tchar> {
        text.bytes().map(|byte| byte as Tchar).chain([0]).collect()
    }

    fn upper(value: Tchar) -> u32 {
        let value = value as u32;
        if (b'a' as u32..=b'z' as u32).contains(&value) {
            value - 32
        } else {
            value
        }
    }

    unsafe extern "C" fn find_value(
        map: *const c_void,
        key: *const c_void,
        out: *mut *const c_void,
    ) -> i32 {
        let state = unsafe { &*map.cast::<State>() };
        for (entry_key, entry_value) in &state.entries {
            let mut actual = key.cast::<Tchar>();
            let mut expected = entry_key.as_ptr();
            loop {
                let actual_char = upper(unsafe { *actual });
                let expected_char = upper(unsafe { *expected });
                if actual_char != expected_char {
                    break;
                }
                if actual_char == 0 {
                    unsafe { *out = entry_value.as_ptr().cast() };
                    return 0;
                }
                actual = unsafe { actual.add(1) };
                expected = unsafe { expected.add(1) };
            }
        }
        unsafe { *out = std::ptr::null() };
        0
    }

    unsafe extern "C" fn assign_result(result: *mut c_void, source: *const c_void) -> i32 {
        let output = unsafe { &mut *result.cast::<Vec<Tchar>>() };
        output.clear();
        let mut source = source.cast::<Tchar>();
        loop {
            let value = unsafe { *source };
            output.push(value);
            if value == 0 {
                return 0;
            }
            source = unsafe { source.add(1) };
        }
    }

    fn format(source: &str) -> Vec<Tchar> {
        let source = wide(source);
        let mut state = State {
            entries: vec![
                (wide("Name"), wide("Harry")),
                (wide("Percent"), wide("100%")),
            ],
            output: Vec::new(),
        };
        let state_ptr = &mut state as *mut State;
        // SAFETY: pointers refer to live state members and remain valid for the call.
        let output = unsafe { &mut (*state_ptr).output as *mut Vec<Tchar> as *mut c_void };
        let services = AppFormatServices {
            find_value,
            assign_result,
        };
        // SAFETY: all source strings are terminated and callbacks use this live state.
        unsafe { app_format(&services, source.as_ptr(), state_ptr.cast(), output).unwrap() };
        state.output
    }

    #[test]
    fn preserves_legacy_substitution_and_unmatched_placeholder_behavior() {
        assert_eq!(
            format("A%NAME% B%percent% C%missing% D%unfinished"),
            wide("AHarry B100% C%missing% D%unfinished")
        );
        assert_eq!(format("100%%"), wide("100%%"));
        assert_eq!(format("end%"), wide("end%"));
    }
}
