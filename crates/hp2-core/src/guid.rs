//! RFC 4122 version-4 GUID shaping from `Core/Src/UnUnix.cpp`.
//!
//! The platform host supplies the 16 random bytes; Rust owns the GUID version
//! and variant bits.

/// Host entropy provider required by GUID generation.
#[repr(C)]
pub struct Hp2GuidServices {
    /// Fill exactly 16 bytes using the platform CSPRNG.
    pub fill_entropy: unsafe extern "C" fn(bytes: *mut u8),
}

/// Fill a 16-byte GUID and set its RFC 4122 version-4 and variant bits.
///
/// # Safety
/// `guid` must be null or point to 16 writable bytes. `services` must remain
/// valid for the duration of the call, and its entropy callback must fill the
/// requested buffer.
pub unsafe fn generate(guid: *mut u8, services: &Hp2GuidServices) {
    if guid.is_null() {
        return;
    }

    unsafe {
        (services.fill_entropy)(guid);
        *guid.add(6) = (*guid.add(6) & 0x0f) | 0x40;
        *guid.add(8) = (*guid.add(8) & 0x3f) | 0x80;
    }
}

#[cfg(test)]
mod tests {
    use super::{Hp2GuidServices, generate};

    unsafe extern "C" fn fill_sequence(bytes: *mut u8) {
        for index in 0..16 {
            // SAFETY: the generation contract supplies a writable 16-byte buffer.
            unsafe { *bytes.add(index) = index as u8 };
        }
    }

    #[test]
    fn generation_preserves_entropy_except_for_version_and_variant_bits() {
        let services = Hp2GuidServices {
            fill_entropy: fill_sequence,
        };
        let mut guid = [0; 16];
        // SAFETY: `guid` is exactly 16 writable bytes.
        unsafe { generate(guid.as_mut_ptr(), &services) };

        assert_eq!(
            guid,
            [0, 1, 2, 3, 4, 5, 0x46, 7, 0x88, 9, 10, 11, 12, 13, 14, 15]
        );
    }
}
