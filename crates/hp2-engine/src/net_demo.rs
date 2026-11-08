//! Port of `Engine/Src/UnDemoRec.cpp` (`UDemoRecDriver`, `UDemoRecConnection`).
//!
//! Every engine call goes through `Hp2NetDemoServices` (`Hp2NetDemoServices.h`). Each
//! thunk returns an `int` status: 0 on success, 1 when the C++ call threw. A thrown
//! status becomes `Err(Thrown)` here; the Rust function returns it with `?`, the FFI
//! export returns status 1, and the C++ shell rethrows the recorded exception through
//! `Hp2RethrowPending`. Results come back through out-pointers (`out`).
//!
//! Divergences: none in logic. `UDemoRecDriver::GetDriver` (UDemoRecConnection),
//! `InitBase`, and `PlayedThisTick` are deleted in C++ (no callers, write-only); the
//! Rust side does not carry them.

use std::ffi::{CStr, c_char, c_void};
use std::mem::MaybeUninit;
use std::ptr::null_mut;

use hp2_abi::engine::generated::{
    AActor, APlayerPawn, FArchive, UClient, UDemoRecDriver, UNetConnection, UNetDriver, UViewport,
};

/// `#define PACKETSIZE 512` (UnDemoRec.cpp:12).
pub const PACKETSIZE: usize = 512;

/// `USOCK_*` values (UnConn.h).
pub const USOCK_CLOSED: i32 = 1;
pub const USOCK_PENDING: i32 = 2;
pub const USOCK_OPEN: i32 = 3;

/// `FIXTIME` (Core.h:61): `4294967296.f`.
pub const FIXTIME: f32 = 4_294_967_296.0;

/// NUL-terminated host TCHAR literals; callbacks receive text, never token IDs.
const fn literal<const N: usize>(s: &str) -> [u32; N] {
    assert!(s.len() + 1 == N);
    let mut result = [0; N];
    let mut i = 0;
    while i < s.len() {
        result[i] = s.as_bytes()[i] as u32;
        i += 1;
    }
    result
}

const OPT_3RDPERSON: [u32; 10] = literal("3rdperson");
const OPT_TIMEBASED: [u32; 10] = literal("timebased");
const OPT_NOFRAMECAP: [u32; 11] = literal("noframecap");
const OPT_LOOP: [u32; 5] = literal("loop");
const CMD_DEMOREC: [u32; 8] = literal("DEMOREC");
const CMD_DEMOPLAY: [u32; 9] = literal("DEMOPLAY");
const CMD_STOPDEMO: [u32; 9] = literal("STOPDEMO");

pub(crate) fn wide(s: &str) -> Vec<u32> {
    s.chars().map(u32::from).chain(std::iter::once(0)).collect()
}

/// Read a live NUL-terminated host TCHAR string.
pub(crate) unsafe fn text(p: *const c_void) -> String {
    let mut p = p.cast::<u32>();
    let mut result = String::new();
    unsafe {
        while *p != 0 {
            result.push(char::from_u32(*p).unwrap_or(char::REPLACEMENT_CHARACTER));
            p = p.add(1);
        }
    }
    result
}

const FILE_UNDEMOREC: &CStr = c"HarryPotter2/Unreal/Engine/Src/UnDemoRec.cpp";

/// A C++ exception was recorded by a services thunk. The C++ shell rethrows it.
#[derive(Debug, Clone, Copy)]
pub struct Thrown;

/// Result of a port call: `Err(Thrown)` means the status was 1 and the exception is pending.
pub type R<T> = Result<T, Thrown>;

/// Converts a services status into a result.
#[inline]
pub fn st(status: i32) -> R<()> {
    if status == 0 { Ok(()) } else { Err(Thrown) }
}

/// Calls a services thunk that writes one `T` through its out-pointer on status 0.
/// The buffer is zeroed first, so a result that is not written reads as zero.
///
/// # Safety
/// `f` must only call a services thunk whose out-pointer parameter is `*mut T`.
#[inline]
pub unsafe fn out<T: Copy>(f: impl FnOnce(*mut T) -> i32) -> R<T> {
    let mut v = MaybeUninit::<T>::zeroed();
    st(f(v.as_mut_ptr()))?;
    // SAFETY: the buffer was zeroed, and a status-0 thunk writes a valid `T` into it.
    Ok(unsafe { v.assume_init() })
}

/// Services table for the demo group. Mirrored by hand in `Hp2NetDemoServices.h`
/// (`struct Hp2NetDemoServices`); field order and types must match exactly.
/// Every entry returns `int` status (0 ok, 1 thrown); results go through out-pointers.
#[repr(C)]
pub struct Hp2NetDemoServices {
    pub demo_app_fail_assert: unsafe extern "C" fn(*const c_char, *const c_char, i32) -> i32,
    pub demo_app_seconds: unsafe extern "C" fn(*mut i64) -> i32,
    pub demo_obj_super_destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_ar_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_ar_at_end: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_ar_tell: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_ar_seek: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub demo_ar_serialize: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub demo_ar_write_i32: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub demo_drv_write_time: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_ar_read_i32: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_ar_read_u32: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    pub demo_ar_read_ftime: unsafe extern "C" fn(*mut c_void, *mut i64) -> i32,
    pub demo_ar_read_fstring_discard: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_url_has_option: unsafe extern "C" fn(*const c_void, *const c_void, *mut i32) -> i32,
    pub demo_parse_command: unsafe extern "C" fn(*mut c_void, *const c_void, *mut i32) -> i32,
    pub demo_app_strcpy: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_fstring_new: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    pub demo_fstring_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_fstring_chars: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_fstring_assign: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_localize_error_ansi: unsafe extern "C" fn(*const c_char, *mut *mut c_void) -> i32,
    pub demo_localize_error_wide: unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> i32,
    pub demo_localize_progress_ansi: unsafe extern "C" fn(*const c_char, *mut *mut c_void) -> i32,
    pub demo_debugf_text: unsafe extern "C" fn(*const c_void) -> i32,
    pub demo_debugf_devnet_text: unsafe extern "C" fn(*const c_void) -> i32,
    pub demo_out_logf_text: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_static_load_actor_class: unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> i32,
    pub demo_static_load_player_pawn_class:
        unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> i32,
    pub demo_sys_cache_path: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    pub demo_app_create_temp_filename: unsafe extern "C" fn(*const c_void, *mut c_void) -> i32,
    pub demo_fm_create_reader: unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> i32,
    pub demo_fm_create_writer: unsafe extern "C" fn(*const c_void, *mut *mut c_void) -> i32,
    pub demo_fm_make_directory: unsafe extern "C" fn(*const c_void) -> i32,
    pub demo_fm_file_size: unsafe extern "C" fn(*const c_void, *mut i32) -> i32,
    pub demo_fm_delete: unsafe extern "C" fn(*const c_void) -> i32,
    pub demo_fm_move: unsafe extern "C" fn(*const c_void, *const c_void, *mut i32) -> i32,
    pub demo_codec_new: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    pub demo_codec_add_rle: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_codec_add_bwt: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_codec_add_mtf: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_codec_add_huffman: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_codec_decode: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub demo_codec_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_cache_ini_new: unsafe extern "C" fn(*mut *mut c_void) -> i32,
    pub demo_cache_ini_delete: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_cache_ini_set_string: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        *const c_void,
        *const c_void,
        *const c_void,
    ) -> i32,
    pub bunch: crate::net_bunch::Hp2NetBunchServices,
    pub demo_bunch_write_text: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_bunch_write_guid: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_bunch_set_reliable: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_bunch_is_error: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_bunch_send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_drv_super_init_connect:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut c_void, *mut i32) -> i32,
    pub demo_drv_super_init_listen:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut c_void, *mut i32) -> i32,
    pub demo_drv_set_demo_filename: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_drv_set_time_zero: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_drv_new_connection:
        unsafe extern "C" fn(*mut c_void, *const c_void, *mut *mut c_void) -> i32,
    pub demo_drv_set_loop_url: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_drv_create_reader: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_drv_create_writer: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_drv_demo_filename_chars: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_drv_spectator_class_chars: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_drv_set_spectator_class: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_drv_loop_url_to_fstring: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_drv_listen_add_client: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_drv_super_tick_dispatch: unsafe extern "C" fn(*mut c_void, f32) -> i32,
    pub demo_drv_time_raw: unsafe extern "C" fn(*mut c_void, *mut i64) -> i32,
    pub demo_drv_notify_get_level: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_drv_delete_self: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_drv_master_copy_linkers: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_drv_add_string_property:
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void) -> i32,
    pub demo_level_engine: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_level_netmode: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_level_net_server_package_map:
        unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_level_welcome_player:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> i32,
    pub demo_level_spawn_actor:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub demo_level_clear_demo_rec_driver: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_level_exec: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_engine_build_server_master_map:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub demo_conn_received_raw_packet: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub demo_conn_super_flush_net: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_conn_super_handle_client_player: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_conn_create_file_channel: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_conn_create_control_channel: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_conn_init_out: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_conn_download: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_conn_set_download: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_conn_notify_progress:
        unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, f32) -> i32,
    pub demo_conn_notify_received_file:
        unsafe extern "C" fn(*mut c_void, i32, *const c_void, i32) -> i32,
    pub demo_pawn_actor_count: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_pawn_actor_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub demo_actor_is_player_start: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_pawn_spawn_spectator: unsafe extern "C" fn(
        *mut c_void,
        *mut c_void,
        *const c_void,
        *const c_void,
        *mut *mut c_void,
    ) -> i32,
    pub demo_pawn_engine_client: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_client_viewport0: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_pawn_set_player: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_pawn_set_admin: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_viewport_reset_input: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_pawn_copy_song: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_pawn_set_transition_fade: unsafe extern "C" fn(*mut c_void) -> i32,
    pub demo_viewport_console: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_console_is_time_demo: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_dl_resolve_info: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub demo_info_package_flags: unsafe extern "C" fn(*mut c_void, *mut u32) -> i32,
    pub demo_info_file_size: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub demo_info_url_chars: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_info_parent_name: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_info_guid_string: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_chdl_set_package_index: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub demo_chdl_ch_download: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub demo_chdl_set_ch_download: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub demo_dl_error_virtual: unsafe extern "C" fn(*mut c_void, *const c_void) -> i32,
    pub demo_dl_done_virtual: unsafe extern "C" fn(*mut c_void) -> i32,
}

const _: () = {
    use std::mem::{align_of, offset_of, size_of};
    assert!(size_of::<i64>() == 8);
    assert!(size_of::<i32>() == 4 && size_of::<u32>() == 4);
    assert!(size_of::<[f32; 3]>() == 12 && align_of::<[f32; 3]>() == align_of::<f32>());
    assert!(size_of::<[i32; 3]>() == 12 && align_of::<[i32; 3]>() == align_of::<i32>());
    assert!(size_of::<Hp2NetDemoServices>() == 118 * size_of::<usize>());
    assert!(align_of::<Hp2NetDemoServices>() == align_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_app_fail_assert) == 0 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_app_seconds) == 1 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_obj_super_destroy) == 2 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_delete) == 3 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_at_end) == 4 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_tell) == 5 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_seek) == 6 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_serialize) == 7 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_write_i32) == 8 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_write_time) == 9 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_read_i32) == 10 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_read_u32) == 11 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_ar_read_ftime) == 12 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_ar_read_fstring_discard) == 13 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_url_has_option) == 14 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_parse_command) == 15 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_app_strcpy) == 16 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fstring_new) == 17 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fstring_delete) == 18 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fstring_chars) == 19 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fstring_assign) == 20 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_localize_error_ansi) == 21 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_localize_error_wide) == 22 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_localize_progress_ansi) == 23 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_debugf_text) == 24 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_debugf_devnet_text) == 25 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_out_logf_text) == 26 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_static_load_actor_class) == 27 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_static_load_player_pawn_class) == 28 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_sys_cache_path) == 29 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_app_create_temp_filename) == 30 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_fm_create_reader) == 31 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fm_create_writer) == 32 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fm_make_directory) == 33 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fm_file_size) == 34 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fm_delete) == 35 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_fm_move) == 36 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_new) == 37 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_add_rle) == 38 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_add_bwt) == 39 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_add_mtf) == 40 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_add_huffman) == 41 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_decode) == 42 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_codec_delete) == 43 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_cache_ini_new) == 44 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_cache_ini_delete) == 45 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_cache_ini_set_string) == 46 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, bunch) == 47 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_bunch_write_text) == 52 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_bunch_write_guid) == 53 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_bunch_set_reliable) == 54 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_bunch_is_error) == 55 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_bunch_send) == 56 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_super_init_connect) == 57 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_super_init_listen) == 58 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_set_demo_filename) == 59 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_set_time_zero) == 60 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_new_connection) == 61 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_set_loop_url) == 62 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_create_reader) == 63 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_create_writer) == 64 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_demo_filename_chars) == 65 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_spectator_class_chars) == 66 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_set_spectator_class) == 67 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_loop_url_to_fstring) == 68 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_listen_add_client) == 69 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_super_tick_dispatch) == 70 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_time_raw) == 71 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_notify_get_level) == 72 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_drv_delete_self) == 73 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_master_copy_linkers) == 74 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_drv_add_string_property) == 75 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_level_engine) == 76 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_level_netmode) == 77 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_level_net_server_package_map) == 78 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_level_welcome_player) == 79 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_level_spawn_actor) == 80 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_level_clear_demo_rec_driver) == 81 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_level_exec) == 82 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_engine_build_server_master_map) == 83 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_conn_received_raw_packet) == 84 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_conn_super_flush_net) == 85 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_conn_super_handle_client_player) == 86 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_conn_create_file_channel) == 87 * size_of::<usize>()
    );
    assert!(
        offset_of!(Hp2NetDemoServices, demo_conn_create_control_channel) == 88 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_conn_init_out) == 89 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_conn_download) == 90 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_conn_set_download) == 91 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_conn_notify_progress) == 92 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_conn_notify_received_file) == 93 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_actor_count) == 94 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_actor_at) == 95 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_actor_is_player_start) == 96 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_spawn_spectator) == 97 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_engine_client) == 98 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_client_viewport0) == 99 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_set_player) == 100 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_set_admin) == 101 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_viewport_reset_input) == 102 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_pawn_copy_song) == 103 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_pawn_set_transition_fade) == 104 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_viewport_console) == 105 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_console_is_time_demo) == 106 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_dl_resolve_info) == 107 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_info_package_flags) == 108 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_info_file_size) == 109 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_info_url_chars) == 110 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_info_parent_name) == 111 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_info_guid_string) == 112 * size_of::<usize>());
    assert!(
        offset_of!(Hp2NetDemoServices, demo_chdl_set_package_index) == 113 * size_of::<usize>()
    );
    assert!(offset_of!(Hp2NetDemoServices, demo_chdl_ch_download) == 114 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_chdl_set_ch_download) == 115 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_dl_error_virtual) == 116 * size_of::<usize>());
    assert!(offset_of!(Hp2NetDemoServices, demo_dl_done_virtual) == 117 * size_of::<usize>());
};

/// `FTime` raw ops (Core.h:60-61). `FTime` is a `long long` `v`; `operator-` returns
/// `(v - t.v)/FIXTIME` as a float; `operator+(float)` adds `(TIMETYP)(f*FIXTIME)`.
#[inline]
fn ftime_sub(a: i64, b: i64) -> f32 {
    (a.wrapping_sub(b) as f32) / FIXTIME
}

#[inline]
fn ftime_add_f32(a: i64, f: f32) -> i64 {
    a.wrapping_add((f * FIXTIME) as i64)
}

#[inline]
fn drv_of(this: *mut c_void) -> *mut UDemoRecDriver {
    this.cast()
}

/// `appSeconds()` through the services table.
fn app_seconds(svc: &Hp2NetDemoServices) -> R<i64> {
    // SAFETY: `out` zeroes the buffer; `demo_app_seconds` writes an `i64` on status 0.
    unsafe { out(|p| (svc.demo_app_seconds)(p)) }
}

/// `check(expr)` (UnFile.h:170): the failing branch calls `appFailAssert` through the
/// services table with the verbatim expression text and this file's path.
///
/// # Safety
/// `svc` is the live services table.
unsafe fn check_fail(svc: &Hp2NetDemoServices, expr: &CStr, line: i32) -> R<()> {
    unsafe {
        st((svc.demo_app_fail_assert)(
            expr.as_ptr(),
            FILE_UNDEMOREC.as_ptr(),
            line,
        ))
    }
}

// ---------------------------------------------------------------------------
// UDemoRecDriver
// ---------------------------------------------------------------------------

/// Shared start-of-demo prefix of `InitConnect` and `InitListen`. This was the C++
/// `UDemoRecDriver::InitBase` method; the C++ method is removed and its body lives here.
///
/// # Safety
/// `this` is a live `UDemoRecDriver`; `url` is a live `FURL`.
unsafe fn driver_init_base(svc: &Hp2NetDemoServices, this: *mut c_void, url: *mut c_void) -> R<()> {
    unsafe {
        st((svc.demo_drv_set_demo_filename)(this, url))?;
        st((svc.demo_drv_set_time_zero)(this))?;
        let drv = drv_of(this);
        (*drv).frame_num = 0;
        (*drv).demo_ended = 0;
    }
    Ok(())
}

/// `UDemoRecDriver::InitConnect`. Returns `UBOOL` as `i32`.
///
/// # Safety
/// `this` is a live `UDemoRecDriver`; `notify`, `url`, `err` are live engine pointers.
pub(crate) trait DemoInitOps {
    type Error;
    unsafe fn init_base(&self, this: *mut c_void, url: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn new_connection(&self, this: *mut c_void, url: *mut c_void) -> Result<*mut c_void, Self::Error>;
}

pub(crate) trait DemoConnectOps: DemoInitOps {
    unsafe fn super_init_connect(&self, this: *mut c_void, notify: *mut c_void, url: *mut c_void, err: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn open_reader(&self, this: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn set_open_error(&self, this: *mut c_void, err: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn copy_loop_url(&self, this: *mut c_void, url: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn has_option(&self, url: *mut c_void, option: *const c_void) -> Result<i32, Self::Error>;
}

/// Source-ordered `UDemoRecDriver::InitConnect` policy shared by canonical Core
/// ownership and the legacy forwarding adapter.
pub(crate) unsafe fn driver_init_connect_with<O: DemoConnectOps>(
    ops: &O,
    this: *mut c_void,
    notify: *mut c_void,
    url: *mut c_void,
    err: *mut c_void,
) -> Result<i32, O::Error> {
    unsafe {
        if !ops.super_init_connect(this, notify, url, err)? { return Ok(0); }
        ops.init_base(this, url)?;
        let conn = ops.new_connection(this, url)?;
        (*this.cast::<UNetDriver>()).server_connection = conn;
        (*conn.cast::<UNetConnection>()).current_net_speed = 1_000_000;
        (*conn.cast::<UNetConnection>()).state = USOCK_PENDING;
        if !ops.open_reader(this)? {
            ops.set_open_error(this, err)?;
            return Ok(0);
        }
        ops.copy_loop_url(this, url)?;
        let drv = drv_of(this);
        (*drv).client_third_person = ops.has_option(url, OPT_3RDPERSON.as_ptr().cast())?;
        (*drv).time_based = ops.has_option(url, OPT_TIMEBASED.as_ptr().cast())?;
        (*drv).no_frame_cap = ops.has_option(url, OPT_NOFRAMECAP.as_ptr().cast())?;
        (*drv).r#loop = ops.has_option(url, OPT_LOOP.as_ptr().cast())?;
        Ok(1)
    }
}

struct ForwardConnectOps<'a>(&'a Hp2NetDemoServices);

impl DemoInitOps for ForwardConnectOps<'_> {
    type Error = Thrown;
    unsafe fn init_base(&self, this: *mut c_void, url: *mut c_void) -> R<()> {
        unsafe { driver_init_base(self.0, this, url) }
    }
    unsafe fn new_connection(&self, this: *mut c_void, url: *mut c_void) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_drv_new_connection)(this, url, p)) }
    }
}
impl DemoConnectOps for ForwardConnectOps<'_> {
    unsafe fn super_init_connect(&self, this: *mut c_void, notify: *mut c_void, url: *mut c_void, err: *mut c_void) -> R<bool> {
        unsafe { out::<i32>(|p| (self.0.demo_drv_super_init_connect)(this, notify, url, err, p)).map(|ok| ok != 0) }
    }
    unsafe fn open_reader(&self, this: *mut c_void) -> R<bool> {
        unsafe {
            let file = out::<*mut c_void>(|p| (self.0.demo_drv_create_reader)(this, p))?;
            (*drv_of(this)).file_ar = file;
            Ok(!file.is_null())
        }
    }
    unsafe fn set_open_error(&self, this: *mut c_void, err: *mut c_void) -> R<()> {
        unsafe {
            let filename = out(|p| (self.0.demo_drv_demo_filename_chars)(this, p))?;
            let message = wide(&format!("Couldn't open demo file {} for reading", text(filename)));
            st((self.0.demo_fstring_assign)(err, message.as_ptr().cast()))
        }
    }
    unsafe fn copy_loop_url(&self, this: *mut c_void, url: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_drv_set_loop_url)(this, url)) }
    }
    unsafe fn has_option(&self, url: *mut c_void, option: *const c_void) -> R<i32> {
        unsafe { out(|p| (self.0.demo_url_has_option)(url, option, p)) }
    }
}

pub unsafe fn driver_init_connect(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    notify: *mut c_void,
    url: *mut c_void,
    err: *mut c_void,
) -> R<i32> {
    unsafe { driver_init_connect_with(&ForwardConnectOps(svc), this, notify, url, err) }
}

/// `UDemoRecDriver::InitListen`. Returns `UBOOL` as `i32`.
///
/// # Safety
/// `this` is a live `UDemoRecDriver`; `notify`, `url`, `err` are live engine pointers.
pub(crate) trait DemoListenOps: DemoInitOps {
    unsafe fn super_init_listen(&self, this: *mut c_void, notify: *mut c_void, url: *mut c_void, err: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn init_out(&self, connection: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn open_writer(&self, this: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn add_client(&self, this: *mut c_void, connection: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn set_write_error(&self, this: *mut c_void, err: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn driver_get_level(&self, this: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_engine(&self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn level_netmode(&self, level: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn level_server_package_map(&self, level: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn driver_copy_linkers(&self, this: *mut c_void, source: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn load_spectator_class(&self) -> Result<*mut c_void, Self::Error>;
    unsafe fn spawn_spectator(&self, level: *mut c_void, class: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn build_server_master_map(&self, engine: *mut c_void, driver: *mut c_void, level: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn create_control_channel(&self, connection: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn welcome_player(&self, level: *mut c_void, connection: *mut c_void, optional: *const c_void) -> Result<(), Self::Error>;
}

const CLIENT_DEMO: [u32; 11] = [67, 76, 73, 69, 78, 84, 68, 69, 77, 79, 0];
const SERVER_DEMO: [u32; 11] = [83, 69, 82, 86, 69, 82, 68, 69, 77, 79, 0];

pub(crate) const DEMO_SPECTATOR_CLASS: [u32; 24] = [
    69, 110, 103, 105, 110, 101, 46, 68, 101, 109, 111,
    82, 101, 99, 83, 112, 101, 99, 116, 97, 116, 111, 114, 0,
];
unsafe fn driver_spawn_rec_spectator_with<O: DemoListenOps>(
    ops: &O,
    this: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let class = ops.load_spectator_class()?;
        let level = ops.driver_get_level(this)?;
        ops.spawn_spectator(level, class)
    }
}

/// Source-ordered `UDemoRecDriver::InitListen` policy, shared by canonical Core
/// ownership and the legacy forwarding adapter.
pub(crate) unsafe fn driver_init_listen_with<O: DemoListenOps>(
    ops: &O,
    this: *mut c_void,
    notify: *mut c_void,
    url: *mut c_void,
    err: *mut c_void,
) -> Result<i32, O::Error> {
    unsafe {
        if !ops.super_init_listen(this, notify, url, err)? { return Ok(0); }
        ops.init_base(this, url)?;
        let conn = ops.new_connection(this, url)?;
        (*conn.cast::<UNetConnection>()).current_net_speed = 1_000_000;
        (*conn.cast::<UNetConnection>()).state = USOCK_OPEN;
        ops.init_out(conn)?;
        if !ops.open_writer(this)? {
            // Native code adds the connection before checking a failed archive.
            ops.add_client(this, conn)?;
            ops.set_write_error(this, err)?;
            return Ok(0);
        }
        ops.add_client(this, conn)?;
        let level = ops.driver_get_level(this)?;
        let engine = ops.level_engine(level)?;
        let level = ops.driver_get_level(this)?;
        if ops.level_netmode(level)? == 3 {
            let level = ops.driver_get_level(this)?;
            let map = ops.level_server_package_map(level)?;
            ops.driver_copy_linkers(this, map)?;
        } else {
            driver_spawn_rec_spectator_with(ops, this)?;
            let level = ops.driver_get_level(this)?;
            ops.build_server_master_map(engine, this, level)?;
        }
        ops.create_control_channel(conn)?;
        let level = ops.driver_get_level(this)?;
        let mode_level = ops.driver_get_level(this)?;
        let client = ops.level_netmode(mode_level)? == 3;
        let standalone = !client && ops.level_netmode(mode_level)? == 0;
        let welcome = if client || standalone { CLIENT_DEMO.as_ptr() } else { SERVER_DEMO.as_ptr() };
        ops.welcome_player(level, conn, welcome.cast())?;
        Ok(1)
    }
}

struct ForwardListenOps<'a>(&'a Hp2NetDemoServices);
impl DemoInitOps for ForwardListenOps<'_> {
    type Error = Thrown;
    unsafe fn init_base(&self, this: *mut c_void, url: *mut c_void) -> R<()> { unsafe { driver_init_base(self.0, this, url) } }
    unsafe fn new_connection(&self, this: *mut c_void, url: *mut c_void) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_drv_new_connection)(this, url, p)) }
    }
}
impl DemoListenOps for ForwardListenOps<'_> {
    unsafe fn super_init_listen(&self, this: *mut c_void, notify: *mut c_void, url: *mut c_void, err: *mut c_void) -> R<bool> {
        unsafe { out::<i32>(|p| (self.0.demo_drv_super_init_listen)(this, notify, url, err, p)).map(|ok| ok != 0) }
    }
    unsafe fn init_out(&self, connection: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_conn_init_out)(connection)) }
    }
    unsafe fn open_writer(&self, this: *mut c_void) -> R<bool> {
        unsafe {
            let file = out::<*mut c_void>(|p| (self.0.demo_drv_create_writer)(this, p))?;
            (*drv_of(this)).file_ar = file;
            Ok(!file.is_null())
        }
    }
    unsafe fn add_client(&self, this: *mut c_void, connection: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_drv_listen_add_client)(this, connection)) }
    }
    unsafe fn set_write_error(&self, this: *mut c_void, err: *mut c_void) -> R<()> {
        unsafe {
            let filename = out(|p| (self.0.demo_drv_demo_filename_chars)(this, p))?;
            let message = wide(&format!("Couldn't open demo file {} for writing", text(filename)));
            st((self.0.demo_fstring_assign)(err, message.as_ptr().cast()))
        }
    }
    unsafe fn driver_get_level(&self, this: *mut c_void) -> R<*mut c_void> {
        unsafe { driver_get_level(self.0, this) }
    }
    unsafe fn level_engine(&self, level: *mut c_void) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_level_engine)(level, p)) }
    }
    unsafe fn level_netmode(&self, level: *mut c_void) -> R<i32> {
        unsafe { out(|p| (self.0.demo_level_netmode)(level, p)) }
    }
    unsafe fn level_server_package_map(&self, level: *mut c_void) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_level_net_server_package_map)(level, p)) }
    }
    unsafe fn driver_copy_linkers(&self, this: *mut c_void, source: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_drv_master_copy_linkers)(this, source)) }
    }
    unsafe fn load_spectator_class(&self) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_static_load_actor_class)(DEMO_SPECTATOR_CLASS.as_ptr().cast(), p)) }
    }
    unsafe fn spawn_spectator(&self, level: *mut c_void, class: *mut c_void) -> R<()> {
        unsafe { out(|p| (self.0.demo_level_spawn_actor)(level, class, p)).map(|_: *mut c_void| ()) }
    }
    unsafe fn build_server_master_map(&self, engine: *mut c_void, driver: *mut c_void, level: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_engine_build_server_master_map)(engine, driver, level)) }
    }
    unsafe fn create_control_channel(&self, connection: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_conn_create_control_channel)(connection)) }
    }
    unsafe fn welcome_player(&self, level: *mut c_void, connection: *mut c_void, optional: *const c_void) -> R<()> {
        unsafe { st((self.0.demo_level_welcome_player)(level, connection, optional)) }
    }
}

pub unsafe fn driver_init_listen(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    notify: *mut c_void,
    url: *mut c_void,
    err: *mut c_void,
) -> R<i32> {
    unsafe { driver_init_listen_with(&ForwardListenOps(svc), this, notify, url, err) }
}

/// Operations for the source-ordered `UDemoRecDriver::LowLevelDestroy` policy.
///
/// Implementations provide logging and archive teardown without requiring a native
/// `FArchive` in the canonical Core path.
pub(crate) trait DemoLowLevelDestroyOps {
    type Error;
    unsafe fn log_close(&self) -> Result<(), Self::Error>;
    unsafe fn close_archive(&self, driver: *mut UDemoRecDriver) -> Result<(), Self::Error>;
}

/// # Safety
/// `this` is a live `UDemoRecDriver`.
pub(crate) unsafe fn driver_low_level_destroy_with<O: DemoLowLevelDestroyOps>(
    ops: &O,
    this: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        ops.log_close()?;
        ops.close_archive(drv_of(this))
    }
}

struct ForwardDestroyOps<'a>(&'a Hp2NetDemoServices);
impl DemoLowLevelDestroyOps for ForwardDestroyOps<'_> {
    type Error = Thrown;
    unsafe fn log_close(&self) -> R<()> {
        unsafe { st((self.0.demo_debugf_text)(literal::<26>("Closing down demo driver.").as_ptr().cast())) }
    }
    unsafe fn close_archive(&self, driver: *mut UDemoRecDriver) -> R<()> {
        unsafe {
            if !(*driver).file_ar.is_null() {
                st((self.0.demo_ar_delete)((*driver).file_ar))?;
                (*driver).file_ar = null_mut();
            }
        }
        Ok(())
    }
}

pub unsafe fn driver_low_level_destroy(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    unsafe { driver_low_level_destroy_with(&ForwardDestroyOps(svc), this) }
}

/// Typed archive and execution leaves for the shared playback dispatcher.
pub(crate) trait DemoTickDispatchOps {
    type Error;
    unsafe fn super_tick_dispatch(&self, driver: *mut UDemoRecDriver, delta: f32) -> Result<(), Self::Error>;
    unsafe fn archive_at_end(&self, archive: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn archive_is_error(&self, archive: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn read_i32(&self, archive: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn read_time(&self, archive: *mut c_void) -> Result<i64, Self::Error>;
    unsafe fn tell(&self, archive: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn seek(&self, archive: *mut c_void, position: i32) -> Result<(), Self::Error>;
    fn seconds(&self) -> Result<i64, Self::Error>;
    unsafe fn read_u32(&self, archive: *mut c_void) -> Result<u32, Self::Error>;
    unsafe fn read_bytes(&self, archive: *mut c_void, output: &mut [u8]) -> Result<(), Self::Error>;
    unsafe fn mark_archive_error(&self, archive: *mut c_void) -> Result<(), Self::Error>;
    fn log_read_failure(&self) -> Result<(), Self::Error>;
    unsafe fn received_raw_packet(&self, connection: *mut UNetConnection, packet: &mut [u8]) -> Result<(), Self::Error>;
    unsafe fn play_loop(&self, driver: *mut UDemoRecDriver) -> Result<(), Self::Error>;
}

struct NativeDemoTickOps<'a>(&'a Hp2NetDemoServices);
impl DemoTickDispatchOps for NativeDemoTickOps<'_> {
    type Error = Thrown;
    unsafe fn super_tick_dispatch(&self, driver: *mut UDemoRecDriver, delta: f32) -> R<()> {
        unsafe { st((self.0.demo_drv_super_tick_dispatch)(driver.cast(), delta)) }
    }
    unsafe fn archive_at_end(&self, archive: *mut c_void) -> R<bool> {
        unsafe { out::<i32>(|p| (self.0.demo_ar_at_end)(archive, p)).map(|value| value != 0) }
    }
    unsafe fn archive_is_error(&self, archive: *mut c_void) -> R<bool> {
        Ok(unsafe { (*archive.cast::<FArchive>()).ar_is_error != 0 })
    }
    unsafe fn read_i32(&self, archive: *mut c_void) -> R<i32> {
        unsafe { out(|p| (self.0.demo_ar_read_i32)(archive, p)) }
    }
    unsafe fn read_time(&self, archive: *mut c_void) -> R<i64> {
        unsafe { out(|p| (self.0.demo_ar_read_ftime)(archive, p)) }
    }
    unsafe fn tell(&self, archive: *mut c_void) -> R<i32> {
        unsafe { out(|p| (self.0.demo_ar_tell)(archive, p)) }
    }
    unsafe fn seek(&self, archive: *mut c_void, position: i32) -> R<()> {
        unsafe { st((self.0.demo_ar_seek)(archive, position)) }
    }
    fn seconds(&self) -> R<i64> { app_seconds(self.0) }
    unsafe fn read_u32(&self, archive: *mut c_void) -> R<u32> {
        unsafe { out(|p| (self.0.demo_ar_read_u32)(archive, p)) }
    }
    unsafe fn read_bytes(&self, archive: *mut c_void, output: &mut [u8]) -> R<()> {
        unsafe { st((self.0.demo_ar_serialize)(archive, output.as_mut_ptr().cast(), output.len() as i32)) }
    }
    unsafe fn mark_archive_error(&self, archive: *mut c_void) -> R<()> {
        unsafe { (*archive.cast::<FArchive>()).ar_is_error = 1; }
        Ok(())
    }
    fn log_read_failure(&self) -> R<()> {
        unsafe { st((self.0.demo_debugf_devnet_text)(literal::<32>("Failed to read demo file packet").as_ptr().cast())) }
    }
    unsafe fn received_raw_packet(&self, connection: *mut UNetConnection, packet: &mut [u8]) -> R<()> {
        unsafe { st((self.0.demo_conn_received_raw_packet)(connection.cast(), packet.as_mut_ptr().cast(), packet.len() as i32)) }
    }
    unsafe fn play_loop(&self, driver: *mut UDemoRecDriver) -> R<()> {
        unsafe {
            let level = driver_get_level(self.0, driver.cast())?;
            let url = out(|p| (self.0.demo_fstring_new)(p))?;
            let result = (|| {
                st((self.0.demo_drv_loop_url_to_fstring)(driver.cast(), url))?;
                let chars = out(|p| (self.0.demo_fstring_chars)(url, p))?;
                let command = wide(&format!("DEMOPLAY {}", text(chars)));
                st((self.0.demo_level_exec)(level, command.as_ptr().cast()))
            })();
            let cleanup = st((self.0.demo_fstring_delete)(url));
            result.and(cleanup)
        }
    }
}

/// End-of-demo state transition, including source loop-command execution.
unsafe fn tick_at_end_with<O: DemoTickDispatchOps>(ops: &O, driver: *mut UDemoRecDriver) -> Result<(), O::Error> {
    unsafe {
        let server = (*driver).base.server_connection.cast::<UNetConnection>();
        (*server).state = USOCK_CLOSED;
        (*driver).demo_ended = 1;
        if (*driver).r#loop != 0 { ops.play_loop(driver)?; }
    }
    Ok(())
}

/// Shared `UDemoRecDriver::TickDispatch` policy, without an archive ABI assumption.
///
/// # Safety
/// Driver, archive, and reached connections are live in the supplied backend.
pub(crate) unsafe fn driver_tick_dispatch_with<O: DemoTickDispatchOps>(
    ops: &O, driver: *mut UDemoRecDriver, delta: f32,
) -> Result<(), O::Error> {
    unsafe {
        ops.super_tick_dispatch(driver, delta)?;
        (*driver).frame_num = (*driver).frame_num.wrapping_add(1);
        let server = (*driver).base.server_connection.cast::<UNetConnection>();
        if server.is_null() { return Ok(()); }
        let state = (*server).state;
        if state != USOCK_PENDING && state != USOCK_OPEN { return Ok(()); }

        let mut data = [0u8; PACKETSIZE + 8];
        loop {
            let archive = (*driver).file_ar;
            if ops.archive_at_end(archive)? || ops.archive_is_error(archive)? {
                return tick_at_end_with(ops, driver);
            }
            let server_frame_num = ops.read_i32(archive)?;
            let server_packet_time = ops.read_time(archive)?;
            let time = i64::from_ne_bytes((*driver).base.time);
            if ((*driver).time_based == 0 && server_frame_num > (*driver).frame_num)
                || ((*driver).time_based != 0 && server_packet_time > time)
            {
                let position = ops.tell(archive)?;
                ops.seek(archive, position.wrapping_sub(4).wrapping_sub(8))?;
                break;
            }
            if (*driver).no_frame_cap == 0 && (*driver).time_based == 0 && server_packet_time > time {
                let target = ftime_add_f32(ops.seconds()?, ftime_sub(server_packet_time, time));
                while target > ops.seconds()? {}
            }
            let packet_bytes = ops.read_u32(archive)? as usize;
            if packet_bytes > data.len() {
                ops.mark_archive_error(archive)?;
                ops.log_read_failure()?;
                return tick_at_end_with(ops, driver);
            }
            let packet = &mut data[..packet_bytes];
            ops.read_bytes(archive, packet)?;
            if ops.archive_is_error(archive)? {
                ops.log_read_failure()?;
                return tick_at_end_with(ops, driver);
            }
            let server = (*driver).base.server_connection.cast::<UNetConnection>();
            ops.received_raw_packet(server, packet)?;
            let server = (*driver).base.server_connection.cast::<UNetConnection>();
            if (*server).state == USOCK_PENDING { break; }
        }
    }
    Ok(())
}

/// # Safety
/// `this` is a live native `UDemoRecDriver` and the service table remains live.
pub unsafe fn driver_tick_dispatch(
    svc: &Hp2NetDemoServices, this: *mut c_void, delta: f32,
) -> R<()> {
    unsafe { driver_tick_dispatch_with(&NativeDemoTickOps(svc), this.cast(), delta) }
}

pub(crate) trait DemoExecOps {
    type Error;
    unsafe fn parse_command(&mut self, cursor: &mut *const u32, word: *const u32) -> Result<bool, Self::Error>;
    unsafe fn log_active(&mut self, driver: *mut UDemoRecDriver, playback: bool) -> Result<(), Self::Error>;
    unsafe fn log_stopped(&mut self, driver: *mut UDemoRecDriver) -> Result<(), Self::Error>;
    unsafe fn clear_recording_driver(&mut self, driver: *mut UDemoRecDriver) -> Result<(), Self::Error>;
    unsafe fn delete_driver(&mut self, driver: *mut UDemoRecDriver) -> Result<(), Self::Error>;
}

struct NativeDemoExecOps<'a> {
    services: &'a Hp2NetDemoServices,
    output: *mut c_void,
}

impl DemoExecOps for NativeDemoExecOps<'_> {
    type Error = Thrown;
    unsafe fn parse_command(&mut self, cursor: &mut *const u32, word: *const u32) -> R<bool> {
        let mut raw = (*cursor).cast::<c_void>();
        let result = unsafe { out::<i32>(|p| (self.services.demo_parse_command)(
            (&mut raw as *mut *const c_void).cast(), word.cast(), p,
        ))? };
        *cursor = raw.cast();
        Ok(result != 0)
    }
    unsafe fn log_active(&mut self, driver: *mut UDemoRecDriver, playback: bool) -> R<()> {
        let filename = unsafe { out(|p| (self.services.demo_drv_demo_filename_chars)(driver.cast(), p))? };
        let kind = if playback { "playback" } else { "recording" };
        let message = wide(&format!("Demo {kind} currently active: {}", unsafe { text(filename) }));
        unsafe { st((self.services.demo_out_logf_text)(self.output, message.as_ptr().cast())) }
    }
    unsafe fn log_stopped(&mut self, driver: *mut UDemoRecDriver) -> R<()> {
        let filename = unsafe { out(|p| (self.services.demo_drv_demo_filename_chars)(driver.cast(), p))? };
        let message = wide(&format!("Demo {} stopped ({} frames)", unsafe { text(filename) }, unsafe { (*driver).frame_num }));
        unsafe { st((self.services.demo_out_logf_text)(self.output, message.as_ptr().cast())) }
    }
    unsafe fn clear_recording_driver(&mut self, driver: *mut UDemoRecDriver) -> R<()> {
        unsafe {
            let level = driver_get_level(self.services, driver.cast())?;
            st((self.services.demo_level_clear_demo_rec_driver)(level))
        }
    }
    unsafe fn delete_driver(&mut self, driver: *mut UDemoRecDriver) -> R<()> {
        unsafe { st((self.services.demo_drv_delete_self)(driver.cast())) }
    }
}

/// Shared source command policy. Successful recording STOPDEMO deletes `driver`.
///
/// # Safety
/// Driver and command are live in the backend; caller must not touch the driver
/// after a command that deletes it.
pub(crate) unsafe fn driver_exec_with<O: DemoExecOps>(
    ops: &mut O, driver: *mut UDemoRecDriver, cmd: *const u32,
) -> Result<bool, O::Error> {
    unsafe {
        if (*driver).demo_ended != 0 { return Ok(false); }
        let mut cursor = cmd;
        if ops.parse_command(&mut cursor, CMD_DEMOREC.as_ptr())?
            || ops.parse_command(&mut cursor, CMD_DEMOPLAY.as_ptr())?
        {
            let playback = !(*driver).base.server_connection.is_null();
            ops.log_active(driver, playback)?;
            return Ok(true);
        }
        if ops.parse_command(&mut cursor, CMD_STOPDEMO.as_ptr())? {
            ops.log_stopped(driver)?;
            let server = (*driver).base.server_connection.cast::<UNetConnection>();
            if server.is_null() {
                ops.clear_recording_driver(driver)?;
                ops.delete_driver(driver)?;
            } else {
                (*server).state = USOCK_CLOSED;
            }
            return Ok(true);
        }
    }
    Ok(false)
}

/// # Safety
/// Native driver, command, output device and services are live. STOPDEMO can
/// delete a recording driver; the caller must not subsequently dereference it.
pub unsafe fn driver_exec(
    svc: &Hp2NetDemoServices, this: *mut c_void, cmd: *const c_void, ar: *mut c_void,
) -> R<i32> {
    unsafe {
        driver_exec_with(&mut NativeDemoExecOps { services: svc, output: ar }, this.cast(), cmd.cast())
            .map(i32::from)
    }
}

/// `UDemoRecDriver::GetLevel`.
///
/// # Safety
/// `this` is a live `UDemoRecDriver` with a live `Notify`.
pub unsafe fn driver_get_level(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<*mut c_void> {
    unsafe {
        let level = out::<*mut c_void>(|p| (svc.demo_drv_notify_get_level)(this, p))?;
        if level.is_null() {
            check_fail(svc, c"Notify->NotifyGetLevel()", 355)?;
        }
        out::<*mut c_void>(|p| (svc.demo_drv_notify_get_level)(this, p))
    }
}

/// `UDemoRecDriver::SpawnDemoRecSpectator`.
///
/// # Safety
/// `this` is a live `UDemoRecDriver`.
pub unsafe fn driver_spawn_rec_spectator(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    unsafe { driver_spawn_rec_spectator_with(&ForwardListenOps(svc), this) }
}

// ---------------------------------------------------------------------------
// UDemoRecConnection
// ---------------------------------------------------------------------------

/// `UDemoRecConnection::LowLevelSend`.
///
/// # Safety
/// `this` is a live `UDemoRecConnection`; `data` points to `count` readable bytes.
/// Archive primitives shared by canonical Rust storage and the legacy forwarding path.
pub(crate) trait DemoPacketArchiveOps {
    type Error;
    unsafe fn write_i32(&self, archive: *mut c_void, value: i32) -> Result<(), Self::Error>;
    unsafe fn write_time(&self, driver: *mut UDemoRecDriver, archive: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn write_bytes(&self, archive: *mut c_void, data: *const u8, count: i32) -> Result<(), Self::Error>;
}

/// Source-ordered `UDemoRecConnection::LowLevelSend` policy over archive primitives.
///
/// # Safety
/// `this` is a live `UDemoRecConnection`; `data` points to `count` readable bytes.
pub(crate) unsafe fn conn_low_level_send_with<O: DemoPacketArchiveOps>(
    ops: &O,
    this: *mut UNetConnection,
    data: *const u8,
    count: i32,
) -> Result<(), O::Error> {
    unsafe {
        let driver = (*this).driver.cast::<UDemoRecDriver>();
        if (*driver.cast::<UNetDriver>()).server_connection.is_null() {
            let archive = (*driver).file_ar;
            ops.write_i32(archive, (*driver).frame_num)?;
            ops.write_time(driver, archive)?;
            ops.write_i32(archive, count)?;
            ops.write_bytes(archive, data, count)?;
        }
    }
    Ok(())
}

struct ForwardArchiveOps<'a>(&'a Hp2NetDemoServices);
impl DemoPacketArchiveOps for ForwardArchiveOps<'_> {
    type Error = Thrown;
    unsafe fn write_i32(&self, archive: *mut c_void, value: i32) -> R<()> {
        st(unsafe { (self.0.demo_ar_write_i32)(archive, value) })
    }
    unsafe fn write_time(&self, driver: *mut UDemoRecDriver, archive: *mut c_void) -> R<()> {
        st(unsafe { (self.0.demo_drv_write_time)(driver.cast(), archive) })
    }
    unsafe fn write_bytes(&self, archive: *mut c_void, data: *const u8, count: i32) -> R<()> {
        st(unsafe { (self.0.demo_ar_serialize)(archive, data.cast_mut().cast(), count) })
    }
}

/// `UDemoRecConnection::LowLevelSend` forwarding entrypoint.
///
/// # Safety
/// `this` is a live `UDemoRecConnection`; `data` points to `count` readable bytes.
pub unsafe fn conn_low_level_send(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    data: *mut c_void,
    count: i32,
) -> R<()> {
    unsafe { conn_low_level_send_with(&ForwardArchiveOps(svc), this.cast(), data.cast(), count) }
}

/// `UDemoRecConnection::FlushNet`.
///
/// # Safety
/// `this` is a live `UDemoRecConnection`.
pub unsafe fn conn_flush_net(svc: &Hp2NetDemoServices, this: *mut c_void) -> R<()> {
    unsafe {
        let drv = (*this.cast::<UNetConnection>()).driver;
        let server = (*drv.cast::<UNetDriver>()).server_connection;
        if server.is_null() {
            st((svc.demo_conn_super_flush_net)(this))?;
        }
    }
    Ok(())
}

/// `UDemoRecConnection::HandleClientPlayer`.
///
/// # Safety
/// `this` is a live `UDemoRecConnection`; `pawn` is a live `APlayerPawn`.
pub unsafe fn conn_handle_client_player(
    svc: &Hp2NetDemoServices,
    this: *mut c_void,
    pawn: *mut c_void,
) -> R<()> {
    unsafe { conn_handle_client_player_with(&mut NativeDemoClientPlayerOps(svc), this, pawn) }
}

pub(crate) trait DemoClientPlayerOps {
    type Error;
    fn check_fail(&mut self, expression: &CStr, line: i32) -> Result<(), Self::Error>;
    unsafe fn spectator_name(&mut self, driver: *mut UDemoRecDriver) -> Result<*const u32, Self::Error>;
    unsafe fn set_spectator_name(&mut self, driver: *mut UDemoRecDriver, name: *const u32) -> Result<(), Self::Error>;
    unsafe fn load_spectator_class(&mut self, name: *const u32) -> Result<*mut c_void, Self::Error>;
    unsafe fn actor_count(&mut self, pawn: *mut c_void) -> Result<i32, Self::Error>;
    unsafe fn actor_at(&mut self, pawn: *mut c_void, index: i32) -> Result<*mut c_void, Self::Error>;
    unsafe fn is_player_start(&mut self, actor: *mut c_void) -> Result<bool, Self::Error>;
    unsafe fn spawn_spectator(&mut self, pawn: *mut c_void, class: *mut c_void, location: &[f32; 3], rotation: &[i32; 3]) -> Result<*mut c_void, Self::Error>;
    unsafe fn client(&mut self, pawn: *mut c_void) -> Result<*mut UClient, Self::Error>;
    unsafe fn viewport0(&mut self, client: *mut UClient) -> Result<*mut UViewport, Self::Error>;
    unsafe fn set_player(&mut self, pawn: *mut c_void, viewport: *mut UViewport) -> Result<(), Self::Error>;
    unsafe fn set_admin(&mut self, pawn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn reset_input(&mut self, viewport: *mut UViewport) -> Result<(), Self::Error>;
    unsafe fn base_handle(&mut self, connection: *mut UNetConnection, pawn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn copy_song(&mut self, pawn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn set_transition_fade(&mut self, pawn: *mut c_void) -> Result<(), Self::Error>;
    unsafe fn console(&mut self, viewport: *mut UViewport) -> Result<*mut c_void, Self::Error>;
    unsafe fn is_time_demo(&mut self, console: *mut c_void) -> Result<bool, Self::Error>;
}

/// UDemoRecConnection's override, shared without native object service tables.
///
/// # Safety
/// Connection/driver, actor graph and viewport records remain live through
/// source-ordered script and resource operations.
pub(crate) unsafe fn conn_handle_client_player_with<O: DemoClientPlayerOps>(
    ops: &mut O, this: *mut c_void, pawn: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let mut pawn = pawn;
        let driver = (*this.cast::<UNetConnection>()).driver.cast::<UDemoRecDriver>();
        if (*driver).client_third_person != 0 {
            let name = ops.spectator_name(driver)?;
            if *name == 0 {
                ops.set_spectator_name(driver, literal::<17>("Engine.Spectator").as_ptr())?;
            }
            let name = ops.spectator_name(driver)?;
            let class = ops.load_spectator_class(name)?;
            if class.is_null() { ops.check_fail(c"SpectatorClass", 79)?; }
            let mut location = [0.0; 3];
            let mut rotation = [0; 3];
            let mut index = 0;
            while index < ops.actor_count(pawn)? {
                let actor = ops.actor_at(pawn, index)?;
                if !actor.is_null() && ops.is_player_start(actor)? {
                    let v = (*actor.cast::<AActor>()).location;
                    location = [v.x, v.y, v.z];
                    let r = (*actor.cast::<AActor>()).rotation;
                    rotation = [r.pitch, r.yaw, r.roll];
                    break;
                }
                index += 1;
            }
            pawn = ops.spawn_spectator(pawn, class, &location, &rotation)?;
            if pawn.is_null() { ops.check_fail(c"Pawn", 98)?; }
            let client = ops.client(pawn)?;
            if client.is_null() { ops.check_fail(c"Pawn->XLevel->Engine->Client", 99)?; }
            if (*client).viewports.num == 0 {
                ops.check_fail(c"Pawn->XLevel->Engine->Client->Viewports.Num()", 100)?;
            }
            let viewport = ops.viewport0(client)?;
            (*(*viewport).actor.cast::<APlayerPawn>()).player = null_mut();
            ops.set_player(pawn, viewport)?;
            (*pawn.cast::<AActor>()).role = 4;
            (*pawn.cast::<APlayerPawn>()).show_flags = 0x480c;
            (*pawn.cast::<APlayerPawn>()).rend_map = 5;
            ops.set_admin(pawn)?;
            (*pawn.cast::<AActor>()).set_b_net_owner(true);
            (*pawn.cast::<AActor>()).physics = 4;
            // Unlike the base implementation, this override retains viewport.
            ops.reset_input(viewport)?;
            (*this.cast::<UNetConnection>()).state = USOCK_OPEN;
            (*this.cast::<UNetConnection>()).actor = pawn;
        } else {
            ops.base_handle(this.cast(), pawn)?;
        }
        ops.copy_song(pawn)?;
        ops.set_transition_fade(pawn)?;
        let client = ops.client(pawn)?;
        if client.is_null() { ops.check_fail(c"Pawn->XLevel->Engine->Client", 129)?; }
        if (*client).viewports.num == 0 {
            ops.check_fail(c"Pawn->XLevel->Engine->Client->Viewports.Num()", 130)?;
        }
        let viewport = ops.viewport0(client)?;
        let console = ops.console(viewport)?;
        if !console.is_null() && ops.is_time_demo(console)? {
            (*driver).no_frame_cap = 1;
        }
    }
    Ok(())
}

struct NativeDemoClientPlayerOps<'a>(&'a Hp2NetDemoServices);
impl DemoClientPlayerOps for NativeDemoClientPlayerOps<'_> {
    type Error = Thrown;
    fn check_fail(&mut self, expression: &CStr, line: i32) -> R<()> {
        unsafe { check_fail(self.0, expression, line) }
    }
    unsafe fn spectator_name(&mut self, driver: *mut UDemoRecDriver) -> R<*const u32> {
        unsafe { out(|p| (self.0.demo_drv_spectator_class_chars)(driver.cast(), p)).map(|p| p.cast_const().cast()) }
    }
    unsafe fn set_spectator_name(&mut self, driver: *mut UDemoRecDriver, name: *const u32) -> R<()> {
        unsafe { st((self.0.demo_drv_set_spectator_class)(driver.cast(), name.cast())) }
    }
    unsafe fn load_spectator_class(&mut self, name: *const u32) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_static_load_player_pawn_class)(name.cast(), p)) }
    }
    unsafe fn actor_count(&mut self, pawn: *mut c_void) -> R<i32> {
        unsafe { out(|p| (self.0.demo_pawn_actor_count)(pawn, p)) }
    }
    unsafe fn actor_at(&mut self, pawn: *mut c_void, index: i32) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_pawn_actor_at)(pawn, index, p)) }
    }
    unsafe fn is_player_start(&mut self, actor: *mut c_void) -> R<bool> {
        unsafe { out::<i32>(|p| (self.0.demo_actor_is_player_start)(actor, p)).map(|v| v != 0) }
    }
    unsafe fn spawn_spectator(&mut self, pawn: *mut c_void, class: *mut c_void, location: &[f32; 3], rotation: &[i32; 3]) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_pawn_spawn_spectator)(pawn, class, location.as_ptr().cast(), rotation.as_ptr().cast(), p)) }
    }
    unsafe fn client(&mut self, pawn: *mut c_void) -> R<*mut UClient> {
        unsafe { out(|p| (self.0.demo_pawn_engine_client)(pawn, p)).map(|p| p.cast()) }
    }
    unsafe fn viewport0(&mut self, client: *mut UClient) -> R<*mut UViewport> {
        unsafe { out(|p| (self.0.demo_client_viewport0)(client.cast(), p)).map(|p| p.cast()) }
    }
    unsafe fn set_player(&mut self, pawn: *mut c_void, viewport: *mut UViewport) -> R<()> {
        unsafe { st((self.0.demo_pawn_set_player)(pawn, viewport.cast())) }
    }
    unsafe fn set_admin(&mut self, pawn: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_pawn_set_admin)(pawn)) }
    }
    unsafe fn reset_input(&mut self, viewport: *mut UViewport) -> R<()> {
        unsafe { st((self.0.demo_viewport_reset_input)(viewport.cast())) }
    }
    unsafe fn base_handle(&mut self, connection: *mut UNetConnection, pawn: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_conn_super_handle_client_player)(connection.cast(), pawn)) }
    }
    unsafe fn copy_song(&mut self, pawn: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_pawn_copy_song)(pawn)) }
    }
    unsafe fn set_transition_fade(&mut self, pawn: *mut c_void) -> R<()> {
        unsafe { st((self.0.demo_pawn_set_transition_fade)(pawn)) }
    }
    unsafe fn console(&mut self, viewport: *mut UViewport) -> R<*mut c_void> {
        unsafe { out(|p| (self.0.demo_viewport_console)(viewport.cast(), p)) }
    }
    unsafe fn is_time_demo(&mut self, console: *mut c_void) -> R<bool> {
        unsafe { out::<i32>(|p| (self.0.demo_console_is_time_demo)(console, p)).map(|v| v != 0) }
    }
}
