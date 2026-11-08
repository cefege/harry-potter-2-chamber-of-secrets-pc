//! `FTime` archive bit conversion from `Core/Src/UnMisc.cpp`.

use std::ffi::c_void;

/// C++ operations retained for FTime storage and FArchive byte ordering.
#[repr(C)]
pub struct Hp2FTimeArchiveServices {
    pub time_get_float: unsafe extern "C" fn(time: *const c_void) -> f32,
    pub archive_serialize_u64: unsafe extern "C" fn(ar: *mut c_void, value: *mut u64) -> i32,
    pub time_set_float: unsafe extern "C" fn(time: *mut c_void, value: f32) -> i32,
}

fn pack_float_bits(bits: u32) -> u64 {
    let mantissa = (bits & 0x007f_ffff) as u64;
    let exponent = ((bits >> 23) & 0xff) as u64;
    let sign = (bits >> 31) as u64;
    mantissa | (exponent << 52) | (sign << 63)
}

fn unpack_float_bits(bits: u64) -> u32 {
    let mantissa = (bits & 0x007f_ffff) as u32;
    let exponent = ((bits >> 52) & 0xff) as u32;
    let sign = ((bits >> 63) & 1) as u32;
    mantissa | (exponent << 23) | (sign << 31)
}

/// Serialize FTime's legacy float-bit representation as an archive u64.
///
/// # Safety
/// `ar`, `time`, and callbacks must refer to live objects for the duration of
/// the call. The archive callback must honor Rust's status contract.
pub unsafe fn serialize(ar: *mut c_void, time: *mut c_void, svc: &Hp2FTimeArchiveServices) -> i32 {
    if ar.is_null() || time.is_null() {
        return 1;
    }
    let float = unsafe { (svc.time_get_float)(time) };
    let mut bits = pack_float_bits(float.to_bits());
    let status = unsafe { (svc.archive_serialize_u64)(ar, &mut bits) };
    if status != 0 {
        return status;
    }
    let restored = f32::from_bits(unpack_float_bits(bits));
    unsafe { (svc.time_set_float)(time, restored) }
}

#[cfg(test)]
mod tests {
    use super::{pack_float_bits, unpack_float_bits};

    #[test]
    fn float_bitfields_match_legacy_layout() {
        for bits in [
            0x0000_0000,
            0x3f00_0000,
            0xc0a0_0000,
            0x7f80_0000,
            0x7fc1_2345,
        ] {
            assert_eq!(unpack_float_bits(pack_float_bits(bits)), bits);
        }
        assert_eq!(pack_float_bits(0x3f00_0000), 0x07e0_0000_0000_0000);
        assert_eq!(pack_float_bits(0xbfa0_0000), 0x87f0_0000_0020_0000);
    }
}
