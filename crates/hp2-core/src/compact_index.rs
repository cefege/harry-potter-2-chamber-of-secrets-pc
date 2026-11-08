//! Compact package archive indices from `Core/Src/UnObj.cpp`.

use std::ffi::c_void;

/// Unreal archive operations retained in C++ at the FArchive boundary.
#[repr(C)]
pub struct Hp2CompactIndexServices {
    pub archive_is_loading: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    pub archive_is_saving: unsafe extern "C" fn(ar: *mut c_void) -> i32,
    pub archive_serialize_int: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub archive_serialize_byte: unsafe extern "C" fn(ar: *mut c_void, value: *mut u8) -> i32,
    pub archive_mismatch: unsafe extern "C" fn(value: i32, original: i32) -> i32,
}

/// Decodes one Unreal compact index through a checked byte reader.
pub fn decode<E>(mut read_byte: impl FnMut() -> Result<u8, E>) -> Result<i32, E> {
    let first = read_byte()?;
    let mut value = 0i32;
    if first & 0x40 != 0 {
        let second = read_byte()?;
        if second & 0x80 != 0 {
            let third = read_byte()?;
            if third & 0x80 != 0 {
                let fourth = read_byte()?;
                if fourth & 0x80 != 0 {
                    value = read_byte()? as i32;
                }
                value = value.wrapping_shl(7).wrapping_add((fourth & 0x7f) as i32);
            }
            value = value.wrapping_shl(7).wrapping_add((third & 0x7f) as i32);
        }
        value = value.wrapping_shl(7).wrapping_add((second & 0x7f) as i32);
    }
    value = value.wrapping_shl(6).wrapping_add((first & 0x3f) as i32);
    if first & 0x80 != 0 && value != i32::MIN { value = -value; }
    Ok(value)
}

/// Encodes an index and returns the value represented by bytes accepted by the writer.
pub fn encode<E>(value: i32, mut write_byte: impl FnMut(u8) -> Result<u8, E>) -> Result<i32, E> {
    let mut magnitude = if value < 0 { 0u32.wrapping_sub(value as u32) } else { value as u32 };
    let mut first = (if magnitude < 0x40 { magnitude } else { (magnitude & 0x3f) + 0x40 }) as u8;
    if value < 0 { first |= 0x80; }
    let mut decoded = 0i32;
    first = write_byte(first)?;
    if first & 0x40 != 0 {
        magnitude >>= 6;
        let next = if magnitude < 0x80 { magnitude as u8 } else { ((magnitude & 0x7f) + 0x80) as u8 };
        let second = write_byte(next)?;
        if second & 0x80 != 0 {
            magnitude >>= 7;
            let next = if magnitude < 0x80 { magnitude as u8 } else { ((magnitude & 0x7f) + 0x80) as u8 };
            let third = write_byte(next)?;
            if third & 0x80 != 0 {
                magnitude >>= 7;
                let next = if magnitude < 0x80 { magnitude as u8 } else { ((magnitude & 0x7f) + 0x80) as u8 };
                let fourth = write_byte(next)?;
                if fourth & 0x80 != 0 {
                    magnitude >>= 7;
                    decoded = write_byte(magnitude as u8)? as i32;
                }
                decoded = decoded.wrapping_shl(7).wrapping_add((fourth & 0x7f) as i32);
            }
            decoded = decoded.wrapping_shl(7).wrapping_add((third & 0x7f) as i32);
        }
        decoded = decoded.wrapping_shl(7).wrapping_add((second & 0x7f) as i32);
    }
    decoded = decoded.wrapping_shl(6).wrapping_add((first & 0x3f) as i32);
    if first & 0x80 != 0 && decoded != i32::MIN { decoded = -decoded; }
    Ok(decoded)
}

unsafe fn serialize_byte(ar: *mut c_void, byte: &mut u8, svc: &Hp2CompactIndexServices) -> i32 {
    unsafe { (svc.archive_serialize_byte)(ar, byte) }
}

/// Serialize a signed 32-bit value in Unreal's one-to-five-byte compact form.
///
/// # Safety
/// `ar` and the service callbacks must refer to a live archive and obey the
/// callback status contract for the duration of the call.
pub unsafe fn serialize(ar: *mut c_void, value: *mut i32, svc: &Hp2CompactIndexServices) -> i32 {
    if value.is_null() {
        return 1;
    }
    let loading = unsafe { (svc.archive_is_loading)(ar) } != 0;
    let saving = unsafe { (svc.archive_is_saving)(ar) } != 0;
    if !loading && !saving {
        return unsafe { (svc.archive_serialize_int)(ar, value) };
    }

    let original = if loading { 0 } else { unsafe { *value } };
    unsafe { *value = 0 };
    let decoded = if loading {
        decode(|| {
            let mut byte = 0;
            let status = unsafe { serialize_byte(ar, &mut byte, svc) };
            if status == 0 { Ok(byte) } else { Err(status) }
        })
    } else {
        encode(original, |mut byte| {
            let status = unsafe { serialize_byte(ar, &mut byte, svc) };
            if status == 0 { Ok(byte) } else { Err(status) }
        })
    };
    let decoded = match decoded {
        Ok(decoded) => decoded,
        Err(status) => return status,
    };
    unsafe { *value = decoded };

    if saving && decoded != original {
        return unsafe { (svc.archive_mismatch)(decoded, original) };
    }
    0
}

#[cfg(test)]
mod direct_tests {
    use super::{decode, encode};

    #[test]
    fn multibyte_indices_preserve_wire_bit_positions() {
        let cases: &[(i32, &[u8])] = &[
            (8192, &[0x40, 0x80, 0x01]),
            (8193, &[0x41, 0x80, 0x01]),
            (-8193, &[0xc1, 0x80, 0x01]),
            (16383, &[0x7f, 0xff, 0x01]),
            (1048576, &[0x40, 0x80, 0x80, 0x01]),
            (i32::MAX, &[0x7f, 0xff, 0xff, 0xff, 0x0f]),
            (i32::MIN, &[0xc0, 0x80, 0x80, 0x80, 0x10]),
        ];
        for &(value, wire) in cases {
            let mut cursor = wire.iter().copied();
            assert_eq!(decode(|| cursor.next().ok_or(())).unwrap(), value);
            assert_eq!(cursor.next(), None);
            let mut encoded = Vec::new();
            assert_eq!(encode(value, |byte| {
                encoded.push(byte);
                Ok::<u8, ()>(byte)
            }).unwrap(), value);
            assert_eq!(encoded, wire);
        }
    }

    #[test]
    fn direct_codec_matches_signed_compact_index_values() {
        for value in [0, 1, -1, 63, -63, 64, -64, 8192, -8192, i32::MAX, i32::MIN] {
            let mut bytes = Vec::new();
            let encoded = encode(value, |byte| { bytes.push(byte); Ok::<u8, ()>(byte) }).expect("write compact index");
            assert_eq!(encoded, value, "encoded compact index {value}");
            let mut cursor = 0;
            let decoded = decode(|| {
                let byte = *bytes.get(cursor).ok_or(())?;
                cursor += 1;
                Ok::<u8, ()>(byte)
            }).expect("read compact index");
            assert_eq!(decoded, value, "compact index {value}");
            assert_eq!(cursor, bytes.len(), "compact index width for {value}");
        }
    }
}
