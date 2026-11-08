//! Script object-file and game-save-info archive policy from `UnScript.cpp`.
//! The canonical owner supplies real file archives and Core serialization.

use std::ffi::c_void;

#[repr(C)]
pub struct Hp2ActorFileServices {
    pub context: *mut c_void,
    pub open: unsafe extern "C" fn(
        context: *mut c_void,
        path: *const c_void,
        writing: i32,
        save_slot_path: i32,
        out_archive: *mut *mut c_void,
    ) -> i32,
    pub serialize_object: unsafe extern "C" fn(
        context: *mut c_void,
        archive: *mut c_void,
        object: *mut c_void,
    ) -> i32,
    /// Fields 0..=4 are beans, stars, points, save-point ID and current-level string.
    pub serialize_info_field: unsafe extern "C" fn(
        context: *mut c_void,
        archive: *mut c_void,
        info: *mut c_void,
        field: i32,
    ) -> i32,
    pub close: unsafe extern "C" fn(context: *mut c_void, archive: *mut c_void) -> i32,
}

/// The four legacy script file operations. All reject a null object with false
/// before opening (UE1 checked only LoadInfo and dereferenced null in the others).
#[derive(Clone, Copy)]
pub enum FileOperation {
    SaveObject,
    LoadObject,
    SaveInfo,
    LoadInfo,
}

/// Opens, serializes and closes the legacy archive. A missing file returns
/// false; service errors propagate through the canonical runtime error bridge.
///
/// # Safety
/// Path, object, archive and service pointers must obey the Core engine contracts.
pub unsafe fn transfer_file(
    services: &Hp2ActorFileServices,
    operation: FileOperation,
    path: *const c_void,
    object: *mut c_void,
) -> Result<bool, i32> {
    if object.is_null() {
        return Ok(false);
    }
    let writing = matches!(
        operation,
        FileOperation::SaveObject | FileOperation::SaveInfo
    );
    let info = matches!(operation, FileOperation::SaveInfo | FileOperation::LoadInfo);
    let mut archive = std::ptr::null_mut();
    let status = unsafe {
        (services.open)(
            services.context,
            path,
            writing as i32,
            info as i32,
            &mut archive,
        )
    };
    if status != 0 {
        return Err(status);
    }
    if archive.is_null() {
        return Ok(false);
    }
    if info {
        for field in 0..5 {
            let status = unsafe {
                (services.serialize_info_field)(services.context, archive, object, field)
            };
            if status != 0 {
                return Err(status);
            }
        }
    } else {
        let status = unsafe { (services.serialize_object)(services.context, archive, object) };
        if status != 0 {
            return Err(status);
        }
    }
    let status = unsafe { (services.close)(services.context, archive) };
    if status != 0 {
        return Err(status);
    }
    Ok(true)
}

/*-----------------------------------------------------------------------------
    Publish-once storage for UnScript.cpp's `GActorFileServices` table.
-----------------------------------------------------------------------------*/

static mut FILE_SERVICES: Option<Hp2ActorFileServices> = None;

/// # Safety
/// `value` is the real, immutable `GActorFileServices` table; publication
/// happens once, before any native dispatch, and invokes no callbacks.
pub unsafe fn publish_services(value: Hp2ActorFileServices) {
    unsafe { (&raw mut FILE_SERVICES).write(Some(value)) }
}

/// # Safety
/// Call only after `publish_services` has run.
pub unsafe fn published_services() -> Result<Hp2ActorFileServices, i32> {
    unsafe { (&raw const FILE_SERVICES).read().ok_or(1) }
}
