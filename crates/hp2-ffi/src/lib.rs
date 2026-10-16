//! The single staticlib linked into hp2_game, hp2_ucc, hp2_import and the
//! test executables. Port crates' `extern "C"` APIs are re-exported here so
//! the archive and the generated `hp2_rs.h` cover all of them.

use std::ffi::c_void;

use hp2_ffi_support::{guard, install_log_sink};
use hp2_ini::Profile;

/// Route Rust log records to the engine log. Called from `main` before
/// `appInit`; the C++ sink reads `GLog` per line, so records follow
/// `appInit`'s log redirection. A null sink drops records.
///
/// The sink type is spelled out (not the `LogSink` alias) so cbindgen emits a
/// nullable function pointer rather than an opaque `Option_LogSink`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_set_log(sink: Option<unsafe extern "C" fn(text: *const u8, len: usize)>) {
    guard("hp2_rs_set_log", (), || install_log_sink(sink));
}

/// Record in the engine log that the Rust archive is linked and bridged.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_log_startup() {
    guard("hp2_rs_log_startup", (), || {
        log::info!("hp2-ffi {} linked", env!("CARGO_PKG_VERSION"));
    });
}

/// C++ receiver for one ini-build result, called exactly once per export
/// call: the encoded ini text on success, the error message on failure.
/// UTF-8, not NUL-terminated. `ctx` is whatever pointer the caller passed
/// in (its destination, e.g. a `std::string*`).
pub type IniResultSink = unsafe extern "C" fn(ctx: *mut c_void, text: *const u8, len: usize);

/// Borrows a `(ptr, len)` FFI string as `&str`. `len == 0` returns `""`
/// without reading `ptr` (matches an empty `std::string_view`). A null
/// `ptr` with `len != 0`, or invalid UTF-8, is `None`.
unsafe fn str_from_raw<'a>(ptr: *const u8, len: usize) -> Option<&'a str> {
    if len == 0 {
        return Some("");
    }
    if ptr.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(bytes).ok()
}

/// Invokes `sink` once with `result`'s text (success or error) and reports
/// which it was.
unsafe fn deliver(sink: IniResultSink, ctx: *mut c_void, result: Result<String, String>) -> bool {
    let (ok, text) = match result {
        Ok(text) => (true, text),
        Err(text) => (false, text),
    };
    unsafe { sink(ctx, text.as_ptr(), text.len()) };
    ok
}

/// Builds `System/Default.ini`. Replaces the deleted
/// `HP2Import::Detail::BuildDefaultIni` (`HP2ImportIni.cpp`). Paths are
/// UTF-8, not NUL-terminated. `profile` is `HP2Import::Profile`'s C++
/// ordinal (Safe=0, Full=1, RetailOnly=2). Calls `sink` exactly once; the
/// return value says whether it received the ini text or an error message.
///
/// # Safety
/// `template_path`/`retail_path` must each be valid for reads of their
/// given length (or null with length 0). `sink` must be safe to call with
/// `ctx` and a `(ptr, len)` UTF-8 byte span valid only for the call's
/// duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_build_default(
    template_path: *const u8,
    template_path_len: usize,
    retail_path: *const u8,
    retail_path_len: usize,
    profile: u32,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_build_default", false, move || unsafe {
        let template = match str_from_raw(template_path, template_path_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("template path is not valid UTF-8".to_string()),
                );
            }
        };
        let retail = match str_from_raw(retail_path, retail_path_len) {
            Some(s) => s,
            None => return deliver(sink, ctx, Err("retail path is not valid UTF-8".to_string())),
        };
        let profile = match Profile::from_u32(profile) {
            Some(p) => p,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown ini profile ordinal {profile}")),
                );
            }
        };
        deliver(
            sink,
            ctx,
            hp2_ini::build_default_ini(template, retail, profile),
        )
    })
}

/// Builds `System/DefUser.ini`. Replaces the deleted
/// `HP2Import::Detail::BuildDefUserIni` (`HP2ImportIni.cpp`). Same
/// conventions, and the same safety contract, as
/// [`hp2_rs_ini_build_default`].
///
/// # Safety
/// See [`hp2_rs_ini_build_default`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_build_def_user(
    template_path: *const u8,
    template_path_len: usize,
    retail_path: *const u8,
    retail_path_len: usize,
    profile: u32,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_build_def_user", false, move || unsafe {
        let template = match str_from_raw(template_path, template_path_len) {
            Some(s) => s,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err("template path is not valid UTF-8".to_string()),
                );
            }
        };
        let retail = match str_from_raw(retail_path, retail_path_len) {
            Some(s) => s,
            None => return deliver(sink, ctx, Err("retail path is not valid UTF-8".to_string())),
        };
        let profile = match Profile::from_u32(profile) {
            Some(p) => p,
            None => {
                return deliver(
                    sink,
                    ctx,
                    Err(format!("unknown ini profile ordinal {profile}")),
                );
            }
        };
        deliver(
            sink,
            ctx,
            hp2_ini::build_def_user_ini(template, retail, profile),
        )
    })
}
