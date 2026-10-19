//! `extern "C"` exports for the AStatLog port (`hp2_engine::stat_log`).
//!
//! Engine objects and `FString` addresses cross as `*mut c_void`. The C++ services table
//! (`Hp2StatLogServices`) is passed as `*const c_void` and cast inside each export.
//!
//! Status codes returned by the fallible exports, exported to `hp2_rs.h` as
//! `HP2_STAT_LOG_STATUS_*`: `OK` 0, `CHECK_FAILED` 1 (shell calls `appFailAssert` with its
//! own expression, only when `DO_CHECK`), `FILE_MISSING` 2, `MD5_OVERFLOW` 3 and
//! `ARCHIVE_OVERFLOW` 4 (shell calls `appErrorf`), `PANIC` 5 (a Rust panic; shell calls
//! `appErrorf`).

use std::ffi::c_void;

use hp2_engine::params::TChar;
use hp2_engine::stat_log::{self, StatLogError, StatLogResult, StatLogServices};
use hp2_ffi_support::guard;

pub const HP2_STAT_LOG_STATUS_OK: i32 = 0;
pub const HP2_STAT_LOG_STATUS_CHECK_FAILED: i32 = 1;
pub const HP2_STAT_LOG_STATUS_FILE_MISSING: i32 = 2;
pub const HP2_STAT_LOG_STATUS_MD5_OVERFLOW: i32 = 3;
pub const HP2_STAT_LOG_STATUS_ARCHIVE_OVERFLOW: i32 = 4;
pub const HP2_STAT_LOG_STATUS_PANIC: i32 = 5;

fn status(result: StatLogResult) -> i32 {
    match result {
        Ok(()) => HP2_STAT_LOG_STATUS_OK,
        Err(StatLogError::CheckFailed(_)) => HP2_STAT_LOG_STATUS_CHECK_FAILED,
        Err(StatLogError::FileMissing) => HP2_STAT_LOG_STATUS_FILE_MISSING,
        Err(StatLogError::Md5Overflow) => HP2_STAT_LOG_STATUS_MD5_OVERFLOW,
        Err(StatLogError::ArchiveOverflow) => HP2_STAT_LOG_STATUS_ARCHIVE_OVERFLOW,
    }
}

fn services(svc: *const c_void) -> &'static StatLogServices {
    // SAFETY: the C++ shell passes `&GHp2StatLogServices`, a process-lifetime constant.
    unsafe { svc.cast::<StatLogServices>().as_ref() }.expect("StatLog services pointer is null")
}

/// Copies `len` units from `text`. `len <= 0` gives an empty slice, even for a null pointer.
///
/// # Safety
/// When `len > 0`, `text` must point to `len` readable `TChar` values.
unsafe fn text_slice<'a>(text: *const c_void, len: i32) -> &'a [TChar] {
    if len <= 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(text.cast::<TChar>(), len as usize) }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_execute_local_log_batcher(
    svc: *const c_void,
    this: *mut c_void,
) {
    guard(
        "hp2_rs_engine_stat_log_execute_local_log_batcher",
        (),
        || unsafe { stat_log::execute_local_log_batcher(services(svc), this) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_execute_silent_log_batcher(
    svc: *const c_void,
    this: *mut c_void,
) {
    guard(
        "hp2_rs_engine_stat_log_execute_silent_log_batcher",
        (),
        || unsafe { stat_log::execute_silent_log_batcher(services(svc), this) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_batch_local(svc: *const c_void, this: *mut c_void) {
    guard("hp2_rs_engine_stat_log_batch_local", (), || unsafe {
        stat_log::batch_local(services(svc), this)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_browse_relative_local_url(
    svc: *const c_void,
    url: *mut c_void,
) {
    guard(
        "hp2_rs_engine_stat_log_browse_relative_local_url",
        (),
        || unsafe { stat_log::browse_relative_local_url(services(svc), url) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_execute_world_log_batcher(
    svc: *const c_void,
    this: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_execute_world_log_batcher",
        HP2_STAT_LOG_STATUS_PANIC,
        || status(unsafe { stat_log::execute_world_log_batcher(services(svc), this) }),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_initial_check(
    svc: *const c_void,
    this: *mut c_void,
    game: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_initial_check",
        HP2_STAT_LOG_STATUS_PANIC,
        || status(unsafe { stat_log::initial_check(services(svc), this, game) }),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_log_mutator(
    svc: *const c_void,
    this: *mut c_void,
    mutator: *mut c_void,
) {
    guard("hp2_rs_engine_stat_log_log_mutator", (), || unsafe {
        stat_log::log_mutator(services(svc), this, mutator)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_get_player_checksum(
    svc: *const c_void,
    pawn: *mut c_void,
    checksum: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_get_player_checksum",
        HP2_STAT_LOG_STATUS_PANIC,
        || status(unsafe { stat_log::get_player_checksum(services(svc), pawn, checksum) }),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_open_log(
    svc: *const c_void,
    this: *mut c_void,
    log_dir: *const c_void,
) {
    guard("hp2_rs_engine_stat_log_open_log", (), || unsafe {
        stat_log::open_log(services(svc), this, log_dir.cast::<TChar>())
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_close_log(svc: *const c_void, this: *mut c_void) {
    guard("hp2_rs_engine_stat_log_close_log", (), || unsafe {
        stat_log::close_log(services(svc), this)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_watermark(
    svc: *const c_void,
    this: *mut c_void,
    event: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_watermark",
        HP2_STAT_LOG_STATUS_PANIC,
        || status(unsafe { stat_log::watermark(services(svc), this, event) }),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_get_checksum(
    svc: *const c_void,
    this: *mut c_void,
    checksum: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_get_checksum",
        HP2_STAT_LOG_STATUS_PANIC,
        || status(unsafe { stat_log::get_checksum(services(svc), this, checksum) }),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_file_flush(svc: *const c_void, this: *mut c_void) {
    guard("hp2_rs_engine_stat_log_file_flush", (), || unsafe {
        stat_log::file_flush(services(svc), this)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_file_log(
    svc: *const c_void,
    this: *mut c_void,
    event: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_file_log",
        HP2_STAT_LOG_STATUS_PANIC,
        || status(unsafe { stat_log::file_log(services(svc), this, event) }),
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_checksum_commandlet_main(
    svc: *const c_void,
    file_name: *const c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_checksum_commandlet_main",
        HP2_STAT_LOG_STATUS_PANIC,
        || {
            status(unsafe {
                stat_log::checksum_commandlet_main(services(svc), file_name.cast::<TChar>())
            })
        },
    )
}

/// `appMD5UpdateStringUtf16LE(Context, Text, bAppendCRLF)`. `text` holds `len` units.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_stat_log_md5_update_text(
    svc: *const c_void,
    ctx: *mut c_void,
    text: *const c_void,
    len: i32,
    append_line_ending: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_stat_log_md5_update_text",
        HP2_STAT_LOG_STATUS_PANIC,
        || {
            let text = unsafe { text_slice(text, len) };
            status(unsafe {
                stat_log::md5_update_text(services(svc), ctx, text, append_line_ending != 0)
            })
        },
    )
}
