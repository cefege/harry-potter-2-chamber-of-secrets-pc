//! FDependency archive field order from `Core/Src/UnClass.cpp`.

use std::ffi::c_void;

pub const FIELD_CLASS: i32 = 0;
pub const FIELD_DEEP: i32 = 1;
pub const FIELD_SCRIPT_TEXT_CRC: i32 = 2;

/// C++ owns the dependency and archive; callbacks preserve object-index and
/// archive byte-order behavior.
#[repr(C)]
pub struct Hp2ClassDependencyArchiveServices {
    pub archive_serialize_object: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void) -> i32,
    pub archive_serialize_int: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub archive_serialize_dword: unsafe extern "C" fn(ar: *mut c_void, value: *mut u32) -> i32,
    pub dependency_field: unsafe extern "C" fn(dependency: *mut c_void, field: i32) -> *mut c_void,
}

/// Serializes `Class`, `Deep`, then `ScriptTextCRC`.
///
/// # Safety
/// The dependency, archive, services, and all callback-returned pointers must
/// remain valid for this call. Callbacks must preserve virtual object
/// serialization and archive byte ordering.
pub unsafe fn serialize_dependency(
    ar: *mut c_void,
    dependency: *mut c_void,
    svc: &Hp2ClassDependencyArchiveServices,
) -> i32 {
    if ar.is_null() || dependency.is_null() {
        return 1;
    }

    let class = unsafe { (svc.dependency_field)(dependency, FIELD_CLASS) };
    if class.is_null() {
        return 1;
    }
    let status = unsafe { (svc.archive_serialize_object)(ar, class) };
    if status != 0 {
        return status;
    }

    let deep = unsafe { (svc.dependency_field)(dependency, FIELD_DEEP) }.cast::<i32>();
    if deep.is_null() {
        return 1;
    }
    let status = unsafe { (svc.archive_serialize_int)(ar, deep) };
    if status != 0 {
        return status;
    }

    let crc = unsafe { (svc.dependency_field)(dependency, FIELD_SCRIPT_TEXT_CRC) }.cast::<u32>();
    if crc.is_null() {
        return 1;
    }
    unsafe { (svc.archive_serialize_dword)(ar, crc) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
    struct Dependency {
        class_index: i32,
        deep: i32,
        script_text_crc: u32,
    }

    #[derive(Default)]
    struct Archive {
        loading: bool,
        bytes: Vec<u8>,
        position: usize,
    }

    fn read_or_write(ar: *mut c_void, bytes: &mut [u8]) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        let end = ar.position + bytes.len();
        if ar.loading {
            if end > ar.bytes.len() {
                return 1;
            }
            bytes.copy_from_slice(&ar.bytes[ar.position..end]);
        } else {
            if end > ar.bytes.len() {
                ar.bytes.resize(end, 0);
            }
            ar.bytes[ar.position..end].copy_from_slice(bytes);
        }
        ar.position = end;
        0
    }

    unsafe extern "C" fn serialize_object(ar: *mut c_void, value: *mut c_void) -> i32 {
        serialize_int(ar, value.cast::<i32>())
    }

    unsafe extern "C" fn serialize_int(ar: *mut c_void, value: *mut i32) -> i32 {
        read_or_write(ar, unsafe {
            std::slice::from_raw_parts_mut(value.cast(), 4)
        })
    }

    unsafe extern "C" fn serialize_dword(ar: *mut c_void, value: *mut u32) -> i32 {
        read_or_write(ar, unsafe {
            std::slice::from_raw_parts_mut(value.cast(), 4)
        })
    }

    unsafe extern "C" fn dependency_field(dependency: *mut c_void, field: i32) -> *mut c_void {
        let dependency = unsafe { &mut *dependency.cast::<Dependency>() };
        match field {
            FIELD_CLASS => &mut dependency.class_index as *mut i32 as *mut c_void,
            FIELD_DEEP => &mut dependency.deep as *mut i32 as *mut c_void,
            FIELD_SCRIPT_TEXT_CRC => &mut dependency.script_text_crc as *mut u32 as *mut c_void,
            _ => std::ptr::null_mut(),
        }
    }

    const SERVICES: Hp2ClassDependencyArchiveServices = Hp2ClassDependencyArchiveServices {
        archive_serialize_object: serialize_object,
        archive_serialize_int: serialize_int,
        archive_serialize_dword: serialize_dword,
        dependency_field,
    };

    #[test]
    fn dependency_round_trip_preserves_field_order_and_values() {
        let source = Dependency {
            class_index: -2,
            deep: 1,
            script_text_crc: 0x91a2_b3c4,
        };
        let mut saved = source;
        let mut writer = Archive::default();
        assert_eq!(
            unsafe {
                serialize_dependency(
                    (&mut writer as *mut Archive).cast(),
                    (&mut saved as *mut Dependency).cast(),
                    &SERVICES,
                )
            },
            0
        );
        let expected = [
            (-2i32).to_ne_bytes().as_slice(),
            1i32.to_ne_bytes().as_slice(),
            0x91a2_b3c4u32.to_ne_bytes().as_slice(),
        ]
        .concat();
        assert_eq!(writer.bytes, expected);

        let mut loaded = Dependency::default();
        let mut reader = Archive {
            loading: true,
            bytes: writer.bytes,
            position: 0,
        };
        assert_eq!(
            unsafe {
                serialize_dependency(
                    (&mut reader as *mut Archive).cast(),
                    (&mut loaded as *mut Dependency).cast(),
                    &SERVICES,
                )
            },
            0
        );
        assert_eq!(loaded, source);
        assert_eq!(reader.position, 12);
    }

    #[test]
    fn truncated_dependency_stops_at_the_first_incomplete_field() {
        let mut dependency = Dependency::default();
        let mut archive = Archive {
            loading: true,
            bytes: vec![0; 7],
            position: 0,
        };
        assert_ne!(
            unsafe {
                serialize_dependency(
                    (&mut archive as *mut Archive).cast(),
                    (&mut dependency as *mut Dependency).cast(),
                    &SERVICES,
                )
            },
            0
        );
        assert_eq!(archive.position, 4);
    }
}
