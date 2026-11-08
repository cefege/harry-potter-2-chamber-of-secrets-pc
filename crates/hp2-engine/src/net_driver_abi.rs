//! ABI mirror of `Hp2NetDriverServices` in `Engine/Inc/Hp2NetDriverServices.h`.
//!
//! `S` thunks return zero after completion and nonzero after a captured C++
//! exception. `K` thunks are non-throwing direct leaf operations. Text is
//! NUL-terminated `TCHAR` (`wchar_t`), passed as `*const u32`.

use std::ffi::c_void;
use std::mem::{offset_of, size_of};

/// C++ leaf callbacks reached by the Rust-owned `UNetDriver` /
/// `UPackageMapLevel` methods. Field order and signatures exactly mirror
/// `Hp2NetDriverServices`.
#[repr(C)]
pub struct Hp2NetDriverServices {
    // Archive leaves.
    pub drv_ar_serialize_bits: unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32,
    pub drv_ar_serialize_int: unsafe extern "C" fn(*mut c_void, *mut u32, i32) -> i32,
    pub drv_super_serialize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub drv_serialize_references: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    // Object and index leaves.
    pub drv_max_object_index: unsafe extern "C" fn(*mut c_void) -> i32,
    pub drv_index_to_object: unsafe extern "C" fn(*mut c_void, i32, i32, *mut *mut c_void) -> i32,
    pub drv_object_to_index: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    pub drv_object_is_a: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub drv_log_forged_object: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    // Actor and channel leaves.
    pub drv_cast_actor: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub drv_conn_find_actor_channel: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void,
    pub drv_channel_close_checked: unsafe extern "C" fn(*mut c_void) -> i32,
    pub drv_conn_remove_sent_temporary: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    // Connection and package-map lifetime leaves.
    pub drv_conn_tick: unsafe extern "C" fn(*mut c_void) -> i32,
    pub drv_delete_connection: unsafe extern "C" fn(*mut c_void) -> i32,
    pub drv_delete_package_map: unsafe extern "C" fn(*mut c_void) -> i32,
    pub drv_new_package_map: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    // Driver property and command-line leaves.
    pub drv_find_actor_property: unsafe extern "C" fn(*const u32, *mut *mut c_void) -> i32,
    pub drv_app_cmdline: unsafe extern "C" fn() -> *const u32,
    pub drv_parse_param: unsafe extern "C" fn(*const u32, *const u32) -> i32,
    pub drv_parse_command: unsafe extern "C" fn(*mut *const u32, *const u32) -> i32,
    // Static-constructor property leaves.
    pub drv_static_float: unsafe extern "C" fn(
        *mut c_void,
        *const u32,
        u32,
        i32,
        *const u32,
        u32,
        *mut *mut c_void,
    ) -> i32,
    pub drv_static_int: unsafe extern "C" fn(
        *mut c_void,
        *const u32,
        u32,
        i32,
        *const u32,
        u32,
        *mut *mut c_void,
    ) -> i32,
    pub drv_static_bool: unsafe extern "C" fn(
        *mut c_void,
        *const u32,
        u32,
        i32,
        *const u32,
        u32,
        *mut *mut c_void,
    ) -> i32,
    pub drv_static_array: unsafe extern "C" fn(
        *mut c_void,
        *const u32,
        u32,
        i32,
        *const u32,
        u32,
        *mut *mut c_void,
    ) -> i32,
    pub drv_static_str: unsafe extern "C" fn(*mut c_void, *const u32, u32, *mut *mut c_void) -> i32,
    // Base-class and teardown leaves.
    pub drv_low_level_destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    pub drv_super_destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    // Exec (`SOCKETS`) logging leaves.
    pub drv_exec_logf: unsafe extern "C" fn(*mut c_void, *const u32) -> i32,
    pub drv_exec_log_server: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub drv_exec_log_client: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub drv_exec_log_channel: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}

const PTR: usize = size_of::<usize>();

const _: () = assert!(size_of::<Hp2NetDriverServices>() == 32 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_ar_serialize_bits) == 0);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_max_object_index) == 4 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_cast_actor) == 9 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_conn_tick) == 13 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_find_actor_property) == 17 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_static_float) == 21 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_low_level_destroy) == 26 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_exec_logf) == 28 * PTR);
const _: () = assert!(offset_of!(Hp2NetDriverServices, drv_exec_log_channel) == 31 * PTR);
