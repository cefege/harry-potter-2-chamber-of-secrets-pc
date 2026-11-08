//! FString package archive encoding from `Core/Src/UnMisc.cpp`.
//!
//! Rust owns ANSI/UTF-16 wire selection, Unicode scalar handling, and the
//! serialization sequence. C++ services retain FArchive and FString storage
//! operations whose layouts are Unreal-specific.

use std::ffi::c_void;

use crate::ansi::{self, Tchar};

/// C++ archive/string operations used by the FString serializer.
#[repr(C)]
pub struct Hp2FStringArchiveServices {
    pub context: *mut c_void,
    pub archive_is_saving: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    pub archive_is_loading: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    pub archive_count_string: unsafe extern "C" fn(ar: *mut c_void, string: *mut c_void) -> i32,
    pub archive_serialize_index: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub archive_serialize_byte: unsafe extern "C" fn(ar: *mut c_void, value: *mut u8) -> i32,
    pub archive_serialize_unicode: unsafe extern "C" fn(ar: *mut c_void, value: *mut u16) -> i32,
    pub string_num: unsafe extern "C" fn(string: *const c_void) -> i32,
    pub string_data: unsafe extern "C" fn(string: *mut c_void) -> *mut c_void,
    pub string_resize_for_load: unsafe extern "C" fn(string: *mut c_void, count: i32) -> i32,
    pub string_set_num: unsafe extern "C" fn(string: *mut c_void, count: i32),
    pub string_empty: unsafe extern "C" fn(string: *mut c_void) -> i32,
    pub wire_allocate:
        unsafe extern "C" fn(context: *mut c_void, count: i32, output: *mut *mut u16) -> i32,
    pub tchar_to_ansi: unsafe extern "C" fn(value: i32) -> u8,
    pub ansi_to_tchar: unsafe extern "C" fn(value: u8) -> i32,
    pub string_length_overflow: unsafe extern "C" fn() -> i32,
}

unsafe fn host_scalar(text: *const Tchar, index: &mut i32) -> u32 {
    let raw = unsafe { *text.add(*index as usize) } as i64;
    *index += 1;
    if (0..=0x10ffff).contains(&raw) && !(0xd800..=0xdfff).contains(&raw) {
        raw as u32
    } else {
        0xfffd
    }
}

unsafe fn load_string(
    ar: *mut c_void,
    string: *mut c_void,
    saved_num: i32,
    svc: &Hp2FStringArchiveServices,
) -> i32 {
    let wire_units = saved_num.wrapping_abs();
    if wire_units < 0 {
        return 1;
    }
    let status = unsafe { (svc.string_resize_for_load)(string, wire_units) };
    if status != 0 {
        return status;
    }
    let text = unsafe { (svc.string_data)(string) }.cast::<Tchar>();

    if saved_num >= 0 {
        for index in 0..wire_units {
            let mut byte = 0u8;
            let status = unsafe { (svc.archive_serialize_byte)(ar, &mut byte) };
            if status != 0 {
                return status;
            }
            unsafe { *text.add(index as usize) = (svc.ansi_to_tchar)(byte) as Tchar };
        }
    } else {
        let mut wire = std::ptr::null_mut();
        let status = unsafe { (svc.wire_allocate)(svc.context, wire_units, &mut wire) };
        if status != 0 {
            return status;
        }
        if wire_units != 0 && wire.is_null() {
            return 1;
        }
        for index in 0..wire_units {
            let status =
                unsafe { (svc.archive_serialize_unicode)(ar, &mut *wire.add(index as usize)) };
            if status != 0 {
                return status;
            }
        }

        let units = if wire_units == 0 {
            &[]
        } else {
            // SAFETY: `wire_allocate` returned aligned writable storage for
            // exactly `wire_units` initialized u16 values above.
            unsafe { std::slice::from_raw_parts(wire, wire_units as usize) }
        };
        let mut output = 0usize;
        let result = decode_utf16_to(units, |scalar| {
            unsafe { *text.add(output) = scalar as Tchar };
            output += 1;
            Ok::<(), i32>(())
        });
        if let Err(status) = result { return status; }
        unsafe { (svc.string_set_num)(string, output as i32) };
    }

    let count = unsafe { (svc.string_num)(string) };
    if count == 1 && unsafe { *text } == 0 {
        let status = unsafe { (svc.string_empty)(string) };
        if status != 0 {
            return status;
        }
    }
    0
}

/// Serialize one FString using the legacy ANSI-or-UTF-16 wire format.
///
/// # Safety
/// `ar`, `string`, and the service context must be live Unreal objects for the
/// duration of the call. The callbacks must follow the documented status
/// contract; string data pointers must cover the reported FString length.
pub unsafe fn serialize(
    ar: *mut c_void,
    string: *mut c_void,
    svc: &Hp2FStringArchiveServices,
) -> i32 {
    let status = unsafe { (svc.archive_count_string)(ar, string) };
    if status != 0 {
        return status;
    }

    let mut saved_num = 0i32;
    if unsafe { (svc.archive_is_saving)(ar) } != 0 {
        let length = unsafe { (svc.string_num)(string) };
        let text = unsafe { (svc.string_data)(string) }.cast::<Tchar>();
        if length == 0 || unsafe { ansi::is_pure_ansi(text) } {
            saved_num = length;
        } else {
            let mut index = 0i32;
            let mut wire_units = 0i32;
            while index < length {
                let scalar = unsafe { host_scalar(text, &mut index) };
                let added = if scalar > 0xffff { 2 } else { 1 };
                if wire_units > i32::MAX - added {
                    return unsafe { (svc.string_length_overflow)() };
                }
                wire_units += added;
            }
            saved_num = -wire_units;
        }
    }

    let status = unsafe { (svc.archive_serialize_index)(ar, &mut saved_num) };
    if status != 0 {
        return status;
    }
    if unsafe { (svc.archive_is_loading)(ar) } != 0 {
        return unsafe { load_string(ar, string, saved_num, svc) };
    }

    let length = unsafe { (svc.string_num)(string) };
    let text = unsafe { (svc.string_data)(string) }.cast::<Tchar>();
    if saved_num >= 0 {
        for index in 0..length {
            let mut byte = unsafe { (svc.tchar_to_ansi)(*text.add(index as usize) as i32) };
            let status = unsafe { (svc.archive_serialize_byte)(ar, &mut byte) };
            if status != 0 {
                return status;
            }
        }
    } else {
        let mut index = 0i32;
        while index < length {
            let scalar = unsafe { host_scalar(text, &mut index) };
            let result = encode_utf16_scalar(scalar, |mut unit| {
                let status = unsafe { (svc.archive_serialize_unicode)(ar, &mut unit) };
                if status == 0 { Ok(()) } else { Err(status) }
            });
            if let Err(status) = result { return status; }
        }
    }
    0
}

/// Unit kind read by the owned UTF-32 FString wire decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FStringWireUnit {
    /// One legacy ANSI byte, widened to a Unicode scalar.
    Ansi,
    /// One little-endian UTF-16 code unit.
    Utf16,
}

/// Decodes FString character units while leaving count and archive ownership to the caller.
pub fn decode_utf32<E>(
    saved_num: i32,
    mut read_unit: impl FnMut(FStringWireUnit) -> Result<u16, E>,
) -> Result<Vec<u32>, E> {
    let count = saved_num.unsigned_abs() as usize;
    let mut output = Vec::with_capacity(count);
    if saved_num >= 0 {
        for _ in 0..count { output.push(u32::from(read_unit(FStringWireUnit::Ansi)? as u8)); }
        if output.last() == Some(&0) { output.pop(); }
    } else {
        let mut units = Vec::with_capacity(count);
        for _ in 0..count { units.push(read_unit(FStringWireUnit::Utf16)?); }
        decode_utf16_to(&units, |scalar| {
            output.push(scalar);
            Ok::<(), E>(())
        })?;
        if output.last() == Some(&0) { output.pop(); }
    }
    Ok(output)
}

/// Computes the signed legacy FString count, including its trailing NUL unit.
pub fn utf32_wire_length(text: &[u32]) -> Option<i32> {
    let ansi = text.iter().all(|scalar| *scalar <= 0xff && char::from_u32(*scalar).is_some());
    let units = if ansi {
        text.len().checked_add(1)?
    } else {
        text.iter().try_fold(1usize, |count, scalar| {
            count.checked_add(char::from_u32(*scalar).unwrap_or(char::REPLACEMENT_CHARACTER).len_utf16())
        })?
    };
    let count = i32::try_from(units).ok()?;
    Some(if ansi { count } else { -count })
}

/// Encodes UTF-32 characters as legacy ANSI bytes or UTF-16 units.
pub fn encode_utf32<E>(
    text: &[u32],
    ansi: bool,
    mut write_unit: impl FnMut(FStringWireUnit, u16) -> Result<(), E>,
) -> Result<(), E> {
    if ansi {
        for scalar in text { write_unit(FStringWireUnit::Ansi, *scalar as u16)?; }
        write_unit(FStringWireUnit::Ansi, 0)
    } else {
        for scalar in text {
            encode_utf16_scalar(*scalar, |unit| write_unit(FStringWireUnit::Utf16, unit))?;
        }
        write_unit(FStringWireUnit::Utf16, 0)
    }
}

/// Decodes UTF-16 units to scalars, preserving replacement behavior and output order.
pub fn decode_utf16_to<E>(
    units: &[u16],
    mut write_scalar: impl FnMut(u32) -> Result<(), E>,
) -> Result<usize, E> {
    let mut input = 0usize;
    let mut output = 0usize;
    while input < units.len() {
        let first = u32::from(units[input]);
        input += 1;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            if input < units.len() {
                let second = u32::from(units[input]);
                if (0xdc00..=0xdfff).contains(&second) {
                    input += 1;
                    0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
                } else { 0xfffd }
            } else { 0xfffd }
        } else if (0xdc00..=0xdfff).contains(&first) { 0xfffd } else { first };
        write_scalar(scalar)?;
        output += 1;
    }
    Ok(output)
}

/// Emits one Unicode scalar as one or two UTF-16 code units.
pub fn encode_utf16_scalar<E>(scalar: u32, mut write_unit: impl FnMut(u16) -> Result<(), E>) -> Result<(), E> {
    let character = char::from_u32(scalar).unwrap_or(char::REPLACEMENT_CHARACTER);
    let mut units = [0u16; 2];
    for unit in character.encode_utf16(&mut units).iter() { write_unit(*unit)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FStringWireUnit, encode_utf32, utf32_wire_length};
    use crate::compact_index;

    /// Reconstructs the exact FString wire bytes the C++ `FFixedArchive <<
    /// FString` produced: a compact-index header (the signed `utf32_wire_length`)
    /// followed by ANSI bytes or little-endian UTF-16 units plus a trailing NUL.
    fn wire(text: &[u32]) -> Vec<u8> {
        let saved_num = utf32_wire_length(text).expect("text has a wire length");
        let ansi = saved_num >= 0;
        let mut out = Vec::new();
        compact_index::encode(saved_num, |byte| {
            out.push(byte);
            Ok::<u8, ()>(byte)
        })
        .expect("signed index fits the compact encoding");
        encode_utf32(text, ansi, |unit, value| {
            match unit {
                FStringWireUnit::Ansi => out.push(value as u8),
                FStringWireUnit::Utf16 => {
                    out.push((value & 0xff) as u8);
                    out.push((value >> 8) as u8);
                }
            }
            Ok::<(), ()>(())
        })
        .expect("wire encoding succeeds");
        out
    }

    #[test]
    fn ansi_and_utf16_wire_vectors_match_the_cpp_archive_bytes() {
        // ANSI "HP2" -> 0x04 header, "HP2", NUL.
        assert_eq!(wire(&[0x48, 0x50, 0x32]), [0x04, 0x48, 0x50, 0x32, 0x00]);
        // High-byte ANSI {0xe9} -> 0x02 header, 0xe9, NUL.
        assert_eq!(wire(&[0xe9]), [0x02, 0xe9, 0x00]);
        // UTF-16LE "A\u03a9" -> 0x83 header, A, Omega, NUL (little-endian).
        assert_eq!(
            wire(&[0x41, 0x03a9]),
            [0x83, 0x41, 0x00, 0xa9, 0x03, 0x00, 0x00]
        );
        // Non-BMP U+1F642 -> 0x83 header, surrogate pair D83D DE42, NUL.
        assert_eq!(
            wire(&[0x1f642]),
            [0x83, 0x3d, 0xd8, 0x42, 0xde, 0x00, 0x00]
        );
        // Invalid host scalar 0x110000 -> 0x82 header, U+FFFD, NUL.
        assert_eq!(wire(&[0x110000]), [0x82, 0xfd, 0xff, 0x00, 0x00]);
    }
}
