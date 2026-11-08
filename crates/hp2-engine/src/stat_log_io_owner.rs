//! Core-owned filesystem and MD5 leaves for the shared StatLog policy.

use crate::params::TChar;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::{ffi::c_void, path::PathBuf, ptr, slice};

const FILEWRITE_EVEN_IF_READ_ONLY: u32 = 0x04;

fn archive_error(status: i32) -> RuntimeError {
    RuntimeError::new("Canonical StatLog archive operation failed")
        .with_context(&status.to_string())
}

unsafe fn path_units(path: *const TChar) -> RuntimeResult<PathBuf> {
    if path.is_null() {
        return Err(RuntimeError::new("Null StatLog filename"));
    }
    let length = unsafe { hp2_core::unix_text::tchar_length(path.cast()) };
    let text = unsafe { slice::from_raw_parts(path, length) };
    hp2_core::native_path::path_from_units(hp2_core::native_text::code_units(text))
}

unsafe fn system_path(access: CoreAccess<'_>, path: *const TChar) -> RuntimeResult<PathBuf> {
    let path = unsafe { path_units(path) }?;
    Ok(if path.is_absolute() {
        path
    } else {
        access.data_root().join("System").join(path)
    })
}

pub(super) unsafe extern "C" fn file_size(name: *const TChar) -> i32 {
    crate::stat_log_owner::boundary(-1, |access| {
        let path = unsafe { system_path(*access, name) }?;
        hp2_core::unix_file_manager_owner::file_size(*access, &path)
    })
}

pub(super) unsafe extern "C" fn file_create_writer(name: *const TChar) -> *mut c_void {
    crate::stat_log_owner::boundary(ptr::null_mut(), |access| {
        let path = unsafe { system_path(*access, name) }?;
        let Some(archive) = hp2_core::unix_file_manager_owner::open_archive_with_flags(
            *access,
            &path,
            true,
            FILEWRITE_EVEN_IF_READ_ONLY,
        )?
        else {
            return Ok(ptr::null_mut());
        };
        Ok(Box::into_raw(Box::new(archive)).cast())
    })
}

pub(super) unsafe extern "C" fn file_make_directory(name: *const TChar) {
    crate::stat_log_owner::boundary((), |access| {
        let path = unsafe { system_path(*access, name) }?;
        hp2_core::unix_file_manager_owner::make_directory(*access, &path).map(|_| ())
    });
}

pub(super) unsafe extern "C" fn file_move(dst: *const TChar, src: *const TChar) {
    crate::stat_log_owner::boundary((), |access| {
        let dst = unsafe { system_path(*access, dst) }?;
        let src = unsafe { system_path(*access, src) }?;
        hp2_core::unix_file_manager_owner::move_file_paths(*access, &dst, &src, true, true, true)
            .map(|_| ())
    });
}

pub(super) unsafe extern "C" fn archive_serialize(archive: *mut c_void, data: *const u8, len: i32) {
    crate::stat_log_owner::boundary((), |_| {
        let archive = unsafe { &mut *archive.cast::<hp2_core::runtime_archive::RuntimeArchive>() };
        let status = unsafe { archive.serialize(data.cast_mut().cast(), len) };
        if status == 0 {
            Ok(())
        } else {
            Err(archive_error(status))
        }
    });
}

pub(super) unsafe extern "C" fn archive_flush(archive: *mut c_void) {
    crate::stat_log_owner::boundary((), |_| {
        let archive = unsafe { &mut *archive.cast::<hp2_core::runtime_archive::RuntimeArchive>() };
        archive.flush().map_err(archive_error)
    });
}

pub(super) unsafe extern "C" fn archive_delete(archive: *mut c_void) {
    if archive.is_null() {
        return;
    }
    crate::stat_log_owner::boundary((), |_| {
        let mut archive =
            unsafe { Box::from_raw(archive.cast::<hp2_core::runtime_archive::RuntimeArchive>()) };
        let result = archive.close().map_err(archive_error);
        drop(archive);
        result
    });
}

pub(super) unsafe extern "C" fn md5_context_new() -> *mut c_void {
    let context = unsafe { std::mem::zeroed::<hp2_core::md5::Md5Context>() };
    Box::into_raw(Box::new(context)).cast()
}

pub(super) unsafe extern "C" fn md5_context_init(context: *mut c_void) {
    unsafe { hp2_core::md5::init(context.cast()) };
}

pub(super) unsafe extern "C" fn md5_context_update(
    context: *mut c_void,
    data: *const u8,
    len: i32,
) {
    unsafe { hp2_core::md5::update(context.cast(), data, len) };
}

pub(super) unsafe extern "C" fn md5_context_final(digest: *mut u8, context: *mut c_void) {
    unsafe { hp2_core::md5::final_digest(digest, context.cast()) };
}

pub(super) unsafe extern "C" fn md5_context_delete(context: *mut c_void) {
    if !context.is_null() {
        unsafe { drop(Box::from_raw(context.cast::<hp2_core::md5::Md5Context>())) };
    }
}
