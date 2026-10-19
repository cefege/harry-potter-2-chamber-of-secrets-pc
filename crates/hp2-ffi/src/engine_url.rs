//! `UnURL.cpp` exports (`crates/hp2-engine/src/url.rs`). The C++ `FURL` shells in
//! `Engine/Src/UnURL.cpp` forward here. `svc` is the C++ `Hp2UrlServices` table. Text and
//! object arguments are `void*`, so the C side never depends on the Rust `TChar` width.

use std::ffi::c_void;

use hp2_engine::params::TChar;
use hp2_engine::url::{self, UrlServices};
use hp2_ffi_support::guard;

/// `FURL::StaticInit()` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_static_init(svc: *const c_void) {
    guard("hp2_rs_engine_url_static_init", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::static_init(svc);
    })
}

/// `FURL::StaticExit()` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_static_exit(svc: *const c_void) {
    guard("hp2_rs_engine_url_static_exit", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::static_exit(svc);
    })
}

/// `FURL::FURL(FURL* Base, const TCHAR* TextURL, ETravelType Type)` body; writes into `this`.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `this` must be a writable `FURL`. `base` must be a
/// live `FURL` for Relative and Partial construction. `text` must be a NUL-terminated `TCHAR`
/// string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_construct(
    svc: *const c_void,
    this: *mut c_void,
    base: *const c_void,
    text: *const c_void,
    ty: i32,
) {
    guard("hp2_rs_engine_url_construct", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::construct(svc, this, base, text.cast::<TChar>(), ty);
    })
}

/// `FURL::String(FullyQualified) const` body; writes into the `FString` at `out`.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live `FURL`. `out` must be a live
/// `FString`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_string(
    svc: *const c_void,
    url: *const c_void,
    fully_qualified: i32,
    out: *mut c_void,
) {
    guard("hp2_rs_engine_url_string", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::string(svc, url, fully_qualified != 0, out);
    })
}

/// `FURL::IsInternal() const`.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live `FURL`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_is_internal(
    svc: *const c_void,
    url: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_url_is_internal", 0, move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return 0;
        };
        i32::from(url::is_internal(svc, url))
    })
}

/// `FURL::IsLocalInternal() const`.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live `FURL`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_is_local_internal(
    svc: *const c_void,
    url: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_url_is_local_internal", 0, move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return 0;
        };
        i32::from(url::is_local_internal(svc, url))
    })
}

/// `FURL::AddOption(const TCHAR* Str)` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live, writable `FURL`. `s` must be
/// a NUL-terminated `TCHAR` string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_add_option(
    svc: *const c_void,
    url: *mut c_void,
    s: *const c_void,
) {
    guard("hp2_rs_engine_url_add_option", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::add_option(svc, url, s.cast::<TChar>());
    })
}

/// `FURL::LoadURLConfig(Section, Filename)` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live, writable `FURL`. `section`
/// must be a NUL-terminated `TCHAR` string. `filename` must be a NUL-terminated `TCHAR` string
/// or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_load_config(
    svc: *const c_void,
    url: *mut c_void,
    section: *const c_void,
    filename: *const c_void,
) {
    guard("hp2_rs_engine_url_load_config", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::load_config(svc, url, section.cast::<TChar>(), filename.cast::<TChar>());
    })
}

/// `FURL::SaveURLConfig(Section, Item, Filename) const` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live `FURL`. `section` and `item`
/// must be NUL-terminated `TCHAR` strings. `filename` must be a NUL-terminated `TCHAR` string or
/// null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_save_config(
    svc: *const c_void,
    url: *const c_void,
    section: *const c_void,
    item: *const c_void,
    filename: *const c_void,
) {
    guard("hp2_rs_engine_url_save_config", (), move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return;
        };
        url::save_config(
            svc,
            url,
            section.cast::<TChar>(),
            item.cast::<TChar>(),
            filename.cast::<TChar>(),
        );
    })
}

/// `FURL::HasOption(const TCHAR* Test) const` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live `FURL`. `test` must be a
/// NUL-terminated `TCHAR` string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_has_option(
    svc: *const c_void,
    url: *const c_void,
    test: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_url_has_option", 0, move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return 0;
        };
        i32::from(url::has_option(svc, url, test.cast::<TChar>()))
    })
}

/// `FURL::GetOption(const TCHAR* Match, const TCHAR* Default) const` body. Returns a pointer into
/// the option text or `default`.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `url` must be a live `FURL`. `m` and `default` must
/// be NUL-terminated `TCHAR` strings. The result is valid only while the FURL's options are
/// unchanged.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_get_option(
    svc: *const c_void,
    url: *const c_void,
    m: *const c_void,
    default: *const c_void,
) -> *const c_void {
    guard("hp2_rs_engine_url_get_option", default, move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return default;
        };
        url::get_option(svc, url, m.cast::<TChar>(), default.cast::<TChar>()).cast::<c_void>()
    })
}

/// `FURL::operator==(const FURL& Other) const` body.
///
/// # Safety
/// `svc` must point to a live `UrlServices`. `a` and `b` must be live `FURL` objects.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_equal(
    svc: *const c_void,
    a: *const c_void,
    b: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_url_equal", 0, move || unsafe {
        let Some(svc) = svc.cast::<UrlServices>().as_ref() else {
            return 0;
        };
        i32::from(url::equal(svc, a, b))
    })
}

/// `appFilePathToFURLToken(const TCHAR* Path, TCHAR* Out, INT OutCapacity)` body.
///
/// # Safety
/// `path` must be a NUL-terminated `TCHAR` string or null. When `capacity > 0`, `out` must address
/// `capacity` writable `TCHAR`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_url_file_path_to_token(
    path: *const c_void,
    out: *mut c_void,
    capacity: i32,
) -> i32 {
    guard("hp2_rs_engine_url_file_path_to_token", 0, move || unsafe {
        i32::from(url::file_path_to_token(
            path.cast::<TChar>(),
            out.cast::<TChar>(),
            capacity,
        ))
    })
}
