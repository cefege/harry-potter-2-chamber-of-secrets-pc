//! TCHAR text and native-scope helpers shared by engine kernels that format
//! log/command lines in Rust and borrow C++ stack objects through leaves.

use std::{ffi::c_void, ptr, slice};

use hp2_abi::engine::generated::TArrayRaw;

use crate::ansi::Tchar as TChar;

/// Continuation a native scope leaf invokes with the address of its stack object.
pub type NativeScopeContinuation = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;

// This context contains Rust-owned closure state, never a borrowed native object.
// Taking the closure before invoking it also makes native reentry independent.
/// Runs `body` inside a native scope: `invoke` receives the Rust context and the
/// continuation, constructs its stack object, and resumes with that object.
///
/// # Safety
/// `invoke` must call the continuation at most once, with the context it received.
pub unsafe fn scoped<T, F: FnOnce(*mut c_void) -> Result<T, ()>>(
    invoke: impl FnOnce(*mut c_void, NativeScopeContinuation) -> i32,
    body: F,
) -> Result<T, ()> {
    struct Context<F, T> {
        body: Option<F>,
        result: Option<T>,
    }
    unsafe extern "C" fn resume<T, F: FnOnce(*mut c_void) -> Result<T, ()>>(
        context: *mut c_void,
        local: *mut c_void,
    ) -> i32 {
        let context = context.cast::<Context<F, T>>();
        let body = unsafe { (*context).body.take().unwrap() };
        match body(local) {
            Ok(result) => {
                unsafe {
                    (*context).result = Some(result);
                }
                0
            }
            Err(()) => 1,
        }
    }
    let mut context = Context {
        body: Some(body),
        result: None,
    };
    if invoke((&raw mut context).cast(), resume::<T, F>) != 0 {
        Err(())
    } else {
        Ok(context.result.unwrap())
    }
}

/// Text part for [`fmt`].
pub enum TextPart {
    L(&'static str),
    P(*const u32),
    I(i32),
}

/// Length excluding the terminator; null native text is empty.
///
/// # Safety
/// `p` must be null or a NUL-terminated TCHAR string.
pub unsafe fn text_len(p: *const u32) -> usize {
    let mut len = 0;
    if !p.is_null() {
        while unsafe { *p.add(len) } != 0 {
            len += 1;
        }
    }
    len
}

/// Formats into one allocation, without intermediate FString, text or integer copies.
///
/// # Safety
/// Every `TextPart::P` must be a valid NUL-terminated TCHAR string or null.
pub unsafe fn fmt(parts: &[TextPart]) -> Vec<u32> {
    let capacity = 1 + parts
        .iter()
        .map(|part| match part {
            TextPart::L(text) => text.len(),
            TextPart::P(text) => unsafe { text_len(*text) },
            TextPart::I(_) => 11,
        })
        .sum::<usize>();
    let mut out = Vec::with_capacity(capacity);
    for part in parts {
        match part {
            TextPart::L(text) => out.extend(text.chars().map(|c| c as u32)),
            TextPart::P(text) => {
                let mut p = *text;
                if !p.is_null() {
                    while unsafe { *p } != 0 {
                        out.push(unsafe { *p });
                        p = unsafe { p.add(1) };
                    }
                }
            }
            TextPart::I(value) => {
                let mut digits = [0u32; 11];
                let mut start = digits.len();
                let mut n = value.unsigned_abs();
                loop {
                    start -= 1;
                    digits[start] = b'0' as u32 + n % 10;
                    n /= 10;
                    if n == 0 {
                        break;
                    }
                }
                if *value < 0 {
                    start -= 1;
                    digits[start] = b'-' as u32;
                }
                out.extend_from_slice(&digits[start..]);
            }
        }
    }
    out.push(0);
    out
}

// Every policy keyword is an ASCII literal. Compile-time TCHAR storage avoids
// allocating the same keyword at every native call.
/// NUL-terminated compile-time TCHAR array for an ASCII literal.
#[macro_export]
macro_rules! lit {
    ($text:literal $(,)?) => {{
        const TEXT: [u32; $text.len() + 1] = {
            let bytes = $text.as_bytes();
            let mut out = [0; $text.len() + 1];
            let mut i = 0;
            while i < bytes.len() {
                assert!(bytes[i] < 128);
                out[i] = bytes[i] as u32;
                i += 1;
            }
            out
        };
        &TEXT
    }};
}

/// `**string`: the characters, or `TEXT("")` for an empty array.
///
/// # Safety
/// `string` must address a live `FString`.
pub unsafe fn fstring_text(string: *const TArrayRaw) -> *const u32 {
    let raw = unsafe { ptr::read_unaligned(string) };
    if raw.num != 0 {
        raw.data.cast()
    } else {
        lit!("").as_ptr()
    }
}

/// Borrows UTF-32 storage using the host's signed or unsigned 32-bit `TCHAR`.
/// This preserves every code-unit bit, including embedded NULs, without copying.
pub fn tchar_units(text: &[u32]) -> &[TChar] {
    // SAFETY: every supported TChar is i32 or u32 with the same size/alignment,
    // and both integer types admit every bit pattern.
    unsafe { slice::from_raw_parts(text.as_ptr().cast(), text.len()) }
}

/// Borrows native `TCHAR` storage as unsigned 32-bit code units without copying.
pub fn code_units(text: &[TChar]) -> &[u32] {
    // SAFETY: the same integer-layout invariant as tchar_units applies.
    unsafe { slice::from_raw_parts(text.as_ptr().cast(), text.len()) }
}

/// The `Len()` characters of an `FString`, embedded NULs included.
///
/// # Safety
/// `string` must address a live `FString` that outlives the returned slice.
pub unsafe fn fstring_units<'a>(string: *const TArrayRaw) -> &'a [TChar] {
    let raw = unsafe { ptr::read_unaligned(string) };
    if raw.num > 0 {
        unsafe { slice::from_raw_parts(raw.data.cast::<TChar>(), raw.num as usize - 1) }
    } else {
        &[]
    }
}

/// `string == TEXT("")`: `appStricmp(*string, TEXT(""))==0`.
///
/// # Safety
/// `string` must address a live `FString`.
pub unsafe fn fstring_is_empty(string: *const TArrayRaw) -> bool {
    unsafe { *fstring_text(string) == 0 }
}
