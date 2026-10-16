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

/// Borrows a `(ptr, len)` FFI span as `&[u8]`. `len == 0` returns `&[]`
/// without reading `ptr`. A null `ptr` with `len != 0` is `None`.
unsafe fn bytes_from_raw<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if len == 0 {
        return Some(&[]);
    }
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Invokes `sink` once with `result`'s bytes (success or error-message
/// bytes) and reports which it was.
unsafe fn deliver_bytes(
    sink: IniResultSink,
    ctx: *mut c_void,
    result: Result<Vec<u8>, String>,
) -> bool {
    let (ok, bytes): (bool, Vec<u8>) = match result {
        Ok(bytes) => (true, bytes),
        Err(message) => (false, message.into_bytes()),
    };
    unsafe { sink(ctx, bytes.as_ptr(), bytes.len()) };
    ok
}

/// Opaque handle wrapping one parsed launcher-store ini document
/// (`Launcher.ini`/`Game.ini`/`User.ini`). A plain `pub type` alias of the
/// dependency's `hp2_ini::launcher::Document` is not enough: cbindgen's
/// dependency walker does not resolve a `use … as` import, and a `pub type`
/// alias of a foreign type is not guaranteed to be picked up either, so
/// `hp2_rs.h` ended up declaring `LauncherDocument *` with no matching
/// `typedef`. A newtype struct declared directly in this crate (no
/// `#[repr(C)]`, so it stays opaque) is what cbindgen reliably forward-
/// declares as `typedef struct Hp2RsIniDoc Hp2RsIniDoc;`.
pub struct Hp2RsIniDoc(hp2_ini::launcher::Document);

/// Creates an empty launcher-store ini document (UTF-8, `\n` newline, no
/// lines yet), matching default-constructed `Document document;`. Replaces
/// the deleted C++ `Document` default constructor for the subset described
/// below. Free a non-null result with [`hp2_rs_ini_doc_free`].
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_ini_doc_new() -> *mut Hp2RsIniDoc {
    guard("hp2_rs_ini_doc_new", std::ptr::null_mut(), || {
        Box::into_raw(Box::new(Hp2RsIniDoc(hp2_ini::launcher::Document::new())))
    })
}

/// Decodes a launcher-store ini file (`Launcher.ini`/`Game.ini`/`User.ini`)
/// into a new document handle. Replaces the deleted C++ `Decode`. Returns
/// null on failure, after calling `err_sink` once with the error message.
/// Free a non-null result with [`hp2_rs_ini_doc_free`].
///
/// # Safety
/// `bytes` must be valid for reads of `len` (or null with `len` 0).
/// `err_sink` must be safe to call with `err_ctx` and a UTF-8 byte span
/// valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_decode(
    bytes: *const u8,
    len: usize,
    err_sink: IniResultSink,
    err_ctx: *mut c_void,
) -> *mut Hp2RsIniDoc {
    guard(
        "hp2_rs_ini_doc_decode",
        std::ptr::null_mut(),
        move || unsafe {
            let data = match bytes_from_raw(bytes, len) {
                Some(d) => d,
                None => {
                    deliver(
                        err_sink,
                        err_ctx,
                        Err("ini bytes pointer is null".to_string()),
                    );
                    return std::ptr::null_mut();
                }
            };
            match hp2_ini::launcher::Document::decode(data) {
                Ok(doc) => Box::into_raw(Box::new(Hp2RsIniDoc(doc))),
                Err(message) => {
                    deliver(err_sink, err_ctx, Err(message));
                    std::ptr::null_mut()
                }
            }
        },
    )
}

/// Frees a handle returned by [`hp2_rs_ini_doc_new`] or
/// [`hp2_rs_ini_doc_decode`]. A null handle is a no-op.
///
/// # Safety
/// `handle` must be a pointer this module returned, not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_free(handle: *mut Hp2RsIniDoc) {
    guard("hp2_rs_ini_doc_free", (), move || unsafe {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    })
}

/// Encodes `handle` back to bytes in its original encoding/newline style.
/// Replaces the deleted C++ `Encode`. Calls `sink` exactly once with the
/// encoded bytes on success, or the error message on failure (only the
/// UTF-16 output path can fail); the return value says which.
///
/// # Safety
/// `handle` must be a live pointer from this module. `sink` must be safe
/// to call with `ctx` and a byte span valid only for the call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_encode(
    handle: *const Hp2RsIniDoc,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_doc_encode", false, move || unsafe {
        deliver_bytes(sink, ctx, (*handle).0.encode())
    })
}

/// Looks up `section`/`key` in `handle`; the last matching row wins,
/// matching `Get`'s semantics. Replaces the deleted C++ `Get`. Calls `sink`
/// once with the trimmed value if found; the return value says whether it
/// was found (`sink` is not called otherwise).
///
/// # Safety
/// `handle` must be a live pointer from this module. `section`/`key` must
/// be valid UTF-8 for their given length (or null with length 0). `sink`
/// must be safe to call with `ctx` and a byte span valid only for the
/// call's duration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_get(
    handle: *const Hp2RsIniDoc,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
    sink: IniResultSink,
    ctx: *mut c_void,
) -> bool {
    guard("hp2_rs_ini_doc_get", false, move || unsafe {
        let Some(section) = str_from_raw(section, section_len) else {
            return false;
        };
        let Some(key) = str_from_raw(key, key_len) else {
            return false;
        };
        match (*handle).0.get(section, key) {
            Some(value) => {
                sink(ctx, value.as_ptr(), value.len());
                true
            }
            None => false,
        }
    })
}

/// Sets `section`/`key` to `value` in `handle`: updates the matching row in
/// place, or inserts into the section, or creates the section — see the
/// deleted C++ `Set` for the exact placement rules this preserves. No
/// failure mode.
///
/// # Safety
/// `handle` must be a live pointer from this module. `section`/`key`/
/// `value` must each be valid UTF-8 for their given length (or null with
/// length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_set(
    handle: *mut Hp2RsIniDoc,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
    value: *const u8,
    value_len: usize,
) {
    guard("hp2_rs_ini_doc_set", (), move || unsafe {
        let Some(section) = str_from_raw(section, section_len) else {
            return;
        };
        let Some(key) = str_from_raw(key, key_len) else {
            return;
        };
        let Some(value) = str_from_raw(value, value_len) else {
            return;
        };
        (*handle).0.set(section, key, value);
    })
}

/// Removes every row matching `section`/`key` (case-insensitive) from
/// `handle`, leaving the section header and surrounding lines untouched.
/// Replaces the deleted C++ `Erase`.
///
/// # Safety
/// `handle` must be a live pointer from this module. `section`/`key` must
/// each be valid UTF-8 for their given length (or null with length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_ini_doc_erase(
    handle: *mut Hp2RsIniDoc,
    section: *const u8,
    section_len: usize,
    key: *const u8,
    key_len: usize,
) {
    guard("hp2_rs_ini_doc_erase", (), move || unsafe {
        let Some(section) = str_from_raw(section, section_len) else {
            return;
        };
        let Some(key) = str_from_raw(key, key_len) else {
            return;
        };
        (*handle).0.erase(section, key);
    })
}
