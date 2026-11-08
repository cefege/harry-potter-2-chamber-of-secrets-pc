//! Object `ParseObject(Stream, Match, Class, Dest, Parent)` implementation from `UnMisc.cpp`.

use std::ffi::c_void;

use crate::ansi::{Tchar, Threw};

/// C++ callbacks for the case-insensitive NONE test and UObject lookup.
#[repr(C)]
pub struct ParseObjectServices {
    /// Calls `appStricmp`; nonzero status means a C++ exception was captured.
    pub stricmp: unsafe extern "C" fn(*const c_void, *const c_void, *mut i32) -> i32,
    /// Calls `UObject::StaticFindObject`; nonzero status means an exception was captured.
    pub find_object:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut *mut c_void) -> i32,
}

/// Parses an object path while leaving object-table lookup in the C++ runtime.
///
/// A missing key or unresolved name leaves `destination` unchanged. A found
/// `NONE` value sets it to null and returns success.
///
/// # Safety
/// `services` must remain valid. `stream`, `matched`, and `localized_none`
/// must be NUL-terminated `TCHAR` strings. `class` and `parent` must be valid
/// runtime object pointers accepted by `find_object`; `destination` must be
/// writable. `max_len` must be positive and no larger than the local buffer.
pub unsafe fn parse_object(
    services: &ParseObjectServices,
    stream: *const Tchar,
    matched: *const Tchar,
    class: *mut c_void,
    parent: *mut c_void,
    destination: *mut *mut c_void,
    max_len: i32,
    localized_none: *const Tchar,
) -> Result<bool, Threw> {
    if stream.is_null()
        || matched.is_null()
        || destination.is_null()
        || localized_none.is_null()
        || max_len <= 0
        || max_len > 256
    {
        return Ok(false);
    }

    let mut text = [0 as Tchar; 256];
    // SAFETY: inputs are terminated and the local buffer is large enough for max_len.
    if !unsafe { crate::text_parse::parse_string(stream, matched, text.as_mut_ptr(), max_len) } {
        return Ok(false);
    }

    let mut comparison = 0;
    // SAFETY: both arguments are terminated TCHAR strings and comparison is writable.
    if unsafe { (services.stricmp)(text.as_ptr().cast(), localized_none.cast(), &mut comparison) }
        != 0
    {
        return Err(Threw);
    }
    if comparison == 0 {
        // SAFETY: destination is a writable object-pointer slot.
        unsafe { *destination = std::ptr::null_mut() };
        return Ok(true);
    }

    let mut found = std::ptr::null_mut();
    // SAFETY: class, parent, text and output match the callback's C++ contract.
    if unsafe { (services.find_object)(class, parent, text.as_ptr().cast(), &mut found) } != 0 {
        return Err(Threw);
    }
    if found.is_null() {
        return Ok(false);
    }
    // SAFETY: destination is a writable object-pointer slot.
    unsafe { *destination = found };
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{ParseObjectServices, parse_object};
    use crate::ansi::Tchar;
    use std::ffi::c_void;

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

    unsafe extern "C" fn mock_stricmp(
        left: *const c_void,
        right: *const c_void,
        result: *mut i32,
    ) -> i32 {
        let mut left = left.cast::<Tchar>();
        let mut right = right.cast::<Tchar>();
        loop {
            let a = upper(unsafe { *left });
            let b = upper(unsafe { *right });
            if a != b {
                unsafe { *result = if a < b { -1 } else { 1 } };
                return 0;
            }
            if a == 0 {
                unsafe { *result = 0 };
                return 0;
            }
            left = unsafe { left.add(1) };
            right = unsafe { right.add(1) };
        }
    }

    unsafe extern "C" fn mock_find_object(
        _: *mut c_void,
        _: *mut c_void,
        name: *const c_void,
        result: *mut *mut c_void,
    ) -> i32 {
        let target = wide("Target");
        let mut name = name.cast::<Tchar>();
        let mut expected = target.as_ptr();
        loop {
            let actual = unsafe { *name };
            let wanted = unsafe { *expected };
            if actual != wanted {
                unsafe { *result = std::ptr::null_mut() };
                return 0;
            }
            if actual == 0 {
                unsafe { *result = 0x1234usize as *mut c_void };
                return 0;
            }
            name = unsafe { name.add(1) };
            expected = unsafe { expected.add(1) };
        }
    }

    fn parse(stream: &str, initial: usize) -> (bool, usize) {
        let stream = wide(stream);
        let matched = wide("object=");
        let localized_none = wide("NONE");
        let services = ParseObjectServices {
            stricmp: mock_stricmp,
            find_object: mock_find_object,
        };
        let mut destination = initial as *mut c_void;
        // SAFETY: input strings and output storage live for the call; test
        // callbacks use only these terminated strings and the fake pointer.
        let parsed = unsafe {
            parse_object(
                &services,
                stream.as_ptr(),
                matched.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut destination,
                64,
                localized_none.as_ptr(),
            )
        }
        .unwrap();
        (parsed, destination as usize)
    }

    #[test]
    fn distinguishes_none_missing_and_resolved_objects() {
        assert_eq!(parse("object=nOnE", 0x5678), (true, 0));
        assert_eq!(parse("other=Target", 0x5678), (false, 0x5678));
        assert_eq!(parse("object=Missing", 0x5678), (false, 0x5678));
        assert_eq!(parse("object=Target", 0x5678), (true, 0x1234));
    }
}
