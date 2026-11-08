//! Linker-local FName index serialization from `Core/Inc/UnLinker.h`.

use std::ffi::c_void;

use hp2_abi::engine::generated::FName;

/// C++ owns linker arrays and the archive. Callbacks preserve their native
/// bounds checks, compact-index encoding, and exception behavior.
#[repr(C)]
pub struct Hp2LinkerNameArchiveServices {
    pub archive_serialize_index: unsafe extern "C" fn(ar: *mut c_void, value: *mut i32) -> i32,
    pub name_map_count: unsafe extern "C" fn(linker: *mut c_void) -> i32,
    pub name_map_at: unsafe extern "C" fn(linker: *mut c_void, index: i32) -> *mut c_void,
    pub save_name_index:
        unsafe extern "C" fn(linker: *mut c_void, name_index: i32, output: *mut i32) -> i32,
    pub bad_name_index: unsafe extern "C" fn(linker: *mut c_void, index: i32, count: i32) -> i32,
}

/// Validates a package-local FName index without constructing an engine name.
pub fn validate_local_index<E>(
    index: i32,
    count: i32,
    invalid: impl FnOnce(i32, i32) -> E,
) -> Result<i32, E> {
    if index < 0 || index >= count { Err(invalid(index, count)) } else { Ok(index) }
}

/// Reads a compact linker name index and resolves it through `NameMap`.
///
/// # Safety
/// The archive, linker, name, service table, and any pointers returned by its
/// callbacks must remain valid for this call. Callback implementations must
/// retain C++ archive and exception semantics.
pub unsafe fn load_name(
    ar: *mut c_void,
    linker: *mut c_void,
    name: *mut FName,
    svc: &Hp2LinkerNameArchiveServices,
) -> i32 {
    if ar.is_null() || linker.is_null() || name.is_null() {
        return 1;
    }
    let mut index = 0;
    let status = unsafe { (svc.archive_serialize_index)(ar, &mut index) };
    if status != 0 {
        return status;
    }
    let count = unsafe { (svc.name_map_count)(linker) };
    let index = match validate_local_index(index, count, |index, count| unsafe {
        (svc.bad_name_index)(linker, index, count)
    }) {
        Ok(index) => index,
        Err(status) => return status,
    };
    let source = unsafe { (svc.name_map_at)(linker, index) }.cast::<FName>();
    let Some(source) = (unsafe { source.as_ref() }) else {
        return 1;
    };
    unsafe { (*name).index = source.index };
    0
}

/// Maps an FName's global index to its package-local index and writes it as a
/// compact archive index.
///
/// # Safety
/// The archive, linker, name, service table, and any pointers returned by its
/// callbacks must remain valid for this call. Callback implementations must
/// retain C++ archive and exception semantics.
pub unsafe fn save_name(
    ar: *mut c_void,
    linker: *mut c_void,
    name: *const FName,
    svc: &Hp2LinkerNameArchiveServices,
) -> i32 {
    if ar.is_null() || linker.is_null() || name.is_null() {
        return 1;
    }
    let mut index = 0;
    let status = unsafe { (svc.save_name_index)(linker, (*name).index, &mut index) };
    if status != 0 {
        return status;
    }
    unsafe { (svc.archive_serialize_index)(ar, &mut index) }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestContext {
        names: [FName; 3],
        mapped: [i32; 5],
        archive_index: i32,
        written_index: i32,
        invalid_index: i32,
        invalid_count: i32,
        loading: bool,
    }

    unsafe extern "C" fn archive_index(ar: *mut c_void, value: *mut i32) -> i32 {
        let context = unsafe { &mut *ar.cast::<TestContext>() };
        if context.loading {
            unsafe { *value = context.archive_index };
        } else {
            context.written_index = unsafe { *value };
        }
        0
    }

    unsafe extern "C" fn map_count(linker: *mut c_void) -> i32 {
        let context = unsafe { &*linker.cast::<TestContext>() };
        context.names.len() as i32
    }

    unsafe extern "C" fn map_at(linker: *mut c_void, index: i32) -> *mut c_void {
        let context = unsafe { &mut *linker.cast::<TestContext>() };
        unsafe { context.names.as_mut_ptr().add(index as usize).cast() }
    }

    unsafe extern "C" fn save_index(linker: *mut c_void, name_index: i32, output: *mut i32) -> i32 {
        let context = unsafe { &*linker.cast::<TestContext>() };
        let Some(mapped) = context.mapped.get(name_index as usize) else {
            return 2;
        };
        unsafe { *output = *mapped };
        0
    }

    unsafe extern "C" fn bad_index(linker: *mut c_void, index: i32, count: i32) -> i32 {
        let context = unsafe { &mut *linker.cast::<TestContext>() };
        context.invalid_index = index;
        context.invalid_count = count;
        7
    }

    const SERVICES: Hp2LinkerNameArchiveServices = Hp2LinkerNameArchiveServices {
        archive_serialize_index: archive_index,
        name_map_count: map_count,
        name_map_at: map_at,
        save_name_index: save_index,
        bad_name_index: bad_index,
    };

    fn context(loading: bool, index: i32) -> TestContext {
        TestContext {
            names: [
                FName { index: 40 },
                FName { index: 51 },
                FName { index: 62 },
            ],
            mapped: [0, 4, 8, 12, 19],
            archive_index: index,
            written_index: -1,
            invalid_index: i32::MIN,
            invalid_count: -1,
            loading,
        }
    }

    #[test]
    fn loading_resolves_valid_package_local_index() {
        let mut context = context(true, 2);
        let mut name = FName { index: 999 };
        let status = unsafe {
            load_name(
                (&mut context as *mut TestContext).cast(),
                (&mut context as *mut TestContext).cast(),
                &mut name,
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(name.index, 62);
    }

    #[test]
    fn loading_rejects_negative_and_one_past_end_indexes() {
        for invalid in [-1, 3] {
            let mut context = context(true, invalid);
            let mut name = FName { index: 999 };
            let status = unsafe {
                load_name(
                    (&mut context as *mut TestContext).cast(),
                    (&mut context as *mut TestContext).cast(),
                    &mut name,
                    &SERVICES,
                )
            };
            assert_eq!(status, 7);
            assert_eq!(context.invalid_index, invalid);
            assert_eq!(context.invalid_count, 3);
            assert_eq!(name.index, 999);
        }
    }

    #[test]
    fn saving_maps_global_index_before_compact_archive_write() {
        let mut context = context(false, 0);
        let name = FName { index: 4 };
        let status = unsafe {
            save_name(
                (&mut context as *mut TestContext).cast(),
                (&mut context as *mut TestContext).cast(),
                &name,
                &SERVICES,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(context.written_index, 19);
    }
}
