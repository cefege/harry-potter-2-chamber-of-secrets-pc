//! Field ordering for Core networking metadata archive operators in `UnCoreNet.cpp`.

use std::ffi::c_void;

/// C++ owns archive/record layouts and typed object-pointer dispatch; Rust
/// owns the effective field order.
#[repr(C)]
pub struct Hp2CoreNetArchiveServices {
    pub archive_object: unsafe extern "C" fn(ar: *mut c_void, value: *mut c_void, kind: i32) -> i32,
    pub archive_field_array: unsafe extern "C" fn(ar: *mut c_void, fields: *mut c_void) -> i32,
}

const OBJECT_UOBJECT: i32 = 0;
const OBJECT_ULINKER_LOAD: i32 = 1;
const OBJECT_UFIELD: i32 = 2;
const OBJECT_UCLASS: i32 = 3;

/// Serializes `FPackageInfo`'s `Parent`, then `Linker` pointer.
///
/// # Safety
/// The archive, pointer slots, service table, and callback-owned pointers must
/// remain valid for this call. C++ callbacks preserve archive semantics.
pub unsafe fn serialize_package_info(
    ar: *mut c_void,
    parent: *mut c_void,
    linker: *mut c_void,
    services: &Hp2CoreNetArchiveServices,
) -> i32 {
    if ar.is_null() || parent.is_null() || linker.is_null() {
        return 1;
    }
    unsafe {
        let status = (services.archive_object)(ar, parent, OBJECT_UOBJECT);
        if status != 0 {
            return status;
        }
        (services.archive_object)(ar, linker, OBJECT_ULINKER_LOAD)
    }
}

/// Serializes the sole persistent field in `FFieldNetCache`: `Field`.
///
/// # Safety
/// The archive, pointer slot, service table, and callback-owned pointers must
/// remain valid for this call. C++ callbacks preserve archive semantics.
pub unsafe fn serialize_field_net_cache(
    ar: *mut c_void,
    field: *mut c_void,
    services: &Hp2CoreNetArchiveServices,
) -> i32 {
    if ar.is_null() || field.is_null() {
        return 1;
    }
    unsafe { (services.archive_object)(ar, field, OBJECT_UFIELD) }
}

/// Serializes `FClassNetCache::Class` followed by its `Fields` array. The
/// original C++ operator returns immediately after these fields, so `Super`
/// is intentionally not serialized.
///
/// # Safety
/// The archive, pointer slots, service table, and callback-owned pointers must
/// remain valid for this call. C++ callbacks preserve archive semantics.
pub unsafe fn serialize_class_net_cache(
    ar: *mut c_void,
    class: *mut c_void,
    fields: *mut c_void,
    services: &Hp2CoreNetArchiveServices,
) -> i32 {
    if ar.is_null() || class.is_null() || fields.is_null() {
        return 1;
    }
    unsafe {
        let status = (services.archive_object)(ar, class, OBJECT_UCLASS);
        if status != 0 {
            return status;
        }
        (services.archive_field_array)(ar, fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Object(i32),
        FieldArray,
    }

    #[derive(Default)]
    struct Archive {
        calls: Vec<Call>,
        fail_at: Option<usize>,
        failure: i32,
    }

    unsafe extern "C" fn archive_object(ar: *mut c_void, _value: *mut c_void, kind: i32) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        let index = ar.calls.len();
        ar.calls.push(Call::Object(kind));
        if ar.fail_at == Some(index) {
            ar.failure
        } else {
            0
        }
    }

    unsafe extern "C" fn archive_field_array(ar: *mut c_void, _fields: *mut c_void) -> i32 {
        let ar = unsafe { &mut *ar.cast::<Archive>() };
        let index = ar.calls.len();
        ar.calls.push(Call::FieldArray);
        if ar.fail_at == Some(index) {
            ar.failure
        } else {
            0
        }
    }

    const SERVICES: Hp2CoreNetArchiveServices = Hp2CoreNetArchiveServices {
        archive_object,
        archive_field_array,
    };

    fn archive() -> Archive {
        Archive {
            failure: 17,
            ..Archive::default()
        }
    }

    #[test]
    fn package_info_serializes_parent_before_linker() {
        let mut ar = archive();
        let mut parent = std::ptr::null_mut::<c_void>();
        let mut linker = std::ptr::null_mut::<c_void>();
        let status = unsafe {
            serialize_package_info(
                (&mut ar as *mut Archive).cast(),
                (&mut parent as *mut *mut c_void).cast(),
                (&mut linker as *mut *mut c_void).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(
            ar.calls,
            [
                Call::Object(OBJECT_UOBJECT),
                Call::Object(OBJECT_ULINKER_LOAD)
            ]
        );
    }

    #[test]
    fn class_cache_serializes_class_then_field_array_and_propagates_failure() {
        let mut ar = archive();
        ar.fail_at = Some(1);
        let mut class = std::ptr::null_mut::<c_void>();
        let mut fields = std::ptr::null_mut::<c_void>();
        let status = unsafe {
            serialize_class_net_cache(
                (&mut ar as *mut Archive).cast(),
                (&mut class as *mut *mut c_void).cast(),
                (&mut fields as *mut *mut c_void).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 17);
        assert_eq!(ar.calls, [Call::Object(OBJECT_UCLASS), Call::FieldArray]);
    }

    #[test]
    fn field_cache_uses_typed_field_pointer_callback() {
        let mut ar = archive();
        let mut field = std::ptr::null_mut::<c_void>();
        let status = unsafe {
            serialize_field_net_cache(
                (&mut ar as *mut Archive).cast(),
                (&mut field as *mut *mut c_void).cast(),
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(ar.calls, [Call::Object(OBJECT_UFIELD)]);
    }
}
