//! Native-compatible formatting and logging for direct Core property text operations.

use crate::core_access::{RuntimeError, RuntimeResult};
use crate::property_text::Hp2TextFormat;
use crate::ansi::Tchar;
use std::ffi::c_void;
use std::ptr;
/// Formats one property-text fragment to a legacy-sized destination or UTF-32 output device.
///
/// # Safety
/// `out` is null or points to a live `Vec<u32>`. If `out` is null, `dest` is writable
/// for the formatted text and its terminating zero. Every non-null string pointer is a
/// readable NUL-terminated UTF-32 string.
pub(crate) unsafe fn format(
    out: *mut c_void,
    dest: *mut Tchar,
    kind: Hp2TextFormat,
    a: *const Tchar,
    b: *const Tchar,
    n: i32,
    f: f64,
) -> RuntimeResult<i32> {
    let a_text = if a.is_null() { Vec::new() } else { unsafe { read_utf32(a) }? };
    let b_text = if b.is_null() { Vec::new() } else { unsafe { read_utf32(b) }? };
    let mut units = Vec::new();
    match kind {
        Hp2TextFormat::TextFormatInt => append_ascii(&mut units, &n.to_string()),
        Hp2TextFormat::TextFormatFloat => append_ascii(&mut units, &unsafe { format_float(f)? }),
        Hp2TextFormat::TextFormatIndex => {
            append_ascii(&mut units, "[");
            append_ascii(&mut units, &n.to_string());
            append_ascii(&mut units, "]");
        }
        Hp2TextFormat::TextFormatObject => {
            units.extend_from_slice(&a_text);
            units.push(u32::from(b'\''));
            units.extend_from_slice(&b_text);
            units.push(u32::from(b'\''));
        }
        Hp2TextFormat::TextFormatField => {
            units.extend_from_slice(&a_text);
            units.push(u32::from(b'='));
        }
        Hp2TextFormat::TextFormatArrayField => {
            units.extend_from_slice(&a_text);
            units.push(u32::from(b'['));
            append_ascii(&mut units, &n.to_string());
            append_ascii(&mut units, "]");
            units.push(u32::from(b'='));
        }
        Hp2TextFormat::TextFormatCppObject => {
            append_ascii(&mut units, "class ");
            units.extend_from_slice(&a_text);
            units.push(u32::from(b'*'));
        }
        Hp2TextFormat::TextFormatCppBool => {
            append_ascii(&mut units, " ");
            units.extend_from_slice(&a_text);
            units.extend_from_slice(&b_text);
            append_ascii(&mut units, ":1");
        }
        Hp2TextFormat::TextFormatCppReference => {
            append_ascii(&mut units, "& ");
            units.extend_from_slice(&a_text);
            units.extend_from_slice(&b_text);
        }
        Hp2TextFormat::TextFormatCppValue => {
            units.push(u32::from(b' '));
            units.extend_from_slice(&a_text);
            units.extend_from_slice(&b_text);
        }
        Hp2TextFormat::TextFormatCppPointer => {
            append_ascii(&mut units, "* ");
            units.extend_from_slice(&a_text);
        }
        Hp2TextFormat::TextFormatCppString => {
            append_ascii(&mut units, "NoInit ");
            units.extend_from_slice(&a_text);
            units.extend_from_slice(&b_text);
        }
    }
    let count = i32::try_from(units.len()).map_err(|_| RuntimeError::new("Property text is too long"))?;
    if !out.is_null() {
        // SAFETY: `out` validity and exclusive access are guaranteed by this operation's caller.
        unsafe { &mut *out.cast::<Vec<u32>>() }.extend_from_slice(&units);
    } else {
        if dest.is_null() {
            return Err(RuntimeError::new("Null property text destination"));
        }
        // SAFETY: caller guarantees destination capacity as required by the legacy formatter.
        unsafe {
            ptr::copy_nonoverlapping(units.as_ptr().cast::<Tchar>(), dest, units.len());
            dest.add(units.len()).write(0);
        }
    }
    Ok(count)
}

/// Appends one NUL-terminated text string to the Rust-owned output-device accumulator.
///
/// # Safety
/// `out` points to a live `Vec<u32>` and `text` points to a readable NUL-terminated
/// UTF-32 string. The accumulator must be exclusively accessible for this call.
pub(crate) unsafe fn log(out: *mut c_void, text: *const Tchar) -> RuntimeResult<()> {
    if out.is_null() || text.is_null() {
        return Err(RuntimeError::new("Null property text output"));
    }
    let text = unsafe { read_utf32(text) }?;
    // SAFETY: `out` validity and exclusive access are guaranteed by this operation's caller.
    unsafe { &mut *out.cast::<Vec<u32>>() }.extend(text);
    Ok(())
}

unsafe fn read_utf32(text: *const Tchar) -> RuntimeResult<Vec<u32>> {
    if text.is_null() {
        return Err(RuntimeError::new("Null property text string"));
    }
    let mut output = Vec::new();
    let mut cursor = text;
    loop {
        // SAFETY: caller guarantees readable NUL-terminated TCHAR input.
        let unit = unsafe { cursor.read() };
        if unit == 0 {
            break;
        }
        output.push(unit as u32);
        // SAFETY: the following unit is within the same caller-provided terminated string.
        cursor = unsafe { cursor.add(1) };
    }
    Ok(output)
}

fn append_ascii(output: &mut Vec<u32>, text: &str) {
    output.extend(text.bytes().map(u32::from));
}

unsafe fn format_float(value: f64) -> RuntimeResult<String> {
    const CAPACITY: usize = 128;
    let mut buffer = [0_i8; CAPACITY];
    // SAFETY: format is NUL terminated and buffer is writable for CAPACITY bytes.
    let count = unsafe { libc::snprintf(buffer.as_mut_ptr(), CAPACITY, c"%.7g".as_ptr(), value) };
    if count < 0 || count as usize >= CAPACITY {
        return Err(RuntimeError::new("Unable to format property float"));
    }
    // SAFETY: successful snprintf wrote exactly `count` initialized bytes.
    let bytes = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), count as usize) };
    String::from_utf8(bytes.to_vec()).map_err(|_| RuntimeError::new("Invalid property float format"))
}
