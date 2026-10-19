//! `UnParams.cpp` exports (`crates/hp2-engine/src/params.rs`). The C++
//! `GetFVECTOR`/`GetSUBSTRING`/`GetFROTATOR` shells in `Engine/Src/UnParams.cpp`
//! forward here. `svc` is the C++ `Hp2ParamsServices` table.

use std::ffi::c_void;

use hp2_abi::FVector;
use hp2_abi::engine::generated::FRotator;
use hp2_engine::params::{self, ParamsServices, TChar};
use hp2_ffi_support::guard;

/// `GetFVECTOR(const TCHAR* Stream, FVector& Value)` forward.
///
/// # Safety
/// `svc` must point to a live `ParamsServices`. `stream` must be NUL-terminated.
/// `value` must point to a writable `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_params_get_fvector_stream(
    svc: *const c_void,
    stream: *const c_void,
    value: *mut c_void,
) -> i32 {
    guard(
        "hp2_rs_engine_params_get_fvector_stream",
        0,
        move || unsafe {
            let (Some(svc), Some(value)) = (
                svc.cast::<ParamsServices>().as_ref(),
                value.cast::<FVector>().as_mut(),
            ) else {
                return 0;
            };
            params::get_fvector_stream(svc, stream.cast::<TChar>(), value)
        },
    )
}

/// `GetSUBSTRING(const TCHAR* Stream, const TCHAR* Match, TCHAR* Value, INT MaxLen)` forward.
///
/// # Safety
/// `svc` must point to a live `ParamsServices`. `stream` and `matchp` must be
/// NUL-terminated. `value` must have room for `max_len` `TCHAR`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_params_get_substring(
    svc: *const c_void,
    stream: *const c_void,
    matchp: *const c_void,
    value: *mut c_void,
    max_len: i32,
) -> i32 {
    guard("hp2_rs_engine_params_get_substring", 0, move || unsafe {
        let Some(svc) = svc.cast::<ParamsServices>().as_ref() else {
            return 0;
        };
        params::get_substring(
            svc,
            stream.cast::<TChar>(),
            matchp.cast::<TChar>(),
            value.cast::<TChar>(),
            max_len,
        )
    })
}

/// `GetFVECTOR(const TCHAR* Stream, const TCHAR* Match, FVector& Value)` forward.
///
/// # Safety
/// `svc` must point to a live `ParamsServices`. `stream` and `matchp` must be
/// NUL-terminated. `value` must point to a writable `FVector`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_params_get_fvector(
    svc: *const c_void,
    stream: *const c_void,
    matchp: *const c_void,
    value: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_params_get_fvector", 0, move || unsafe {
        let (Some(svc), Some(value)) = (
            svc.cast::<ParamsServices>().as_ref(),
            value.cast::<FVector>().as_mut(),
        ) else {
            return 0;
        };
        params::get_fvector(svc, stream.cast::<TChar>(), matchp.cast::<TChar>(), value)
    })
}

/// `GetFROTATOR(const TCHAR* Stream, FRotator& Rotation, INT ScaleFactor)` forward.
///
/// # Safety
/// `svc` must point to a live `ParamsServices`. `stream` must be NUL-terminated.
/// `rotation` must point to a writable `FRotator`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_params_get_frotator_stream(
    svc: *const c_void,
    stream: *const c_void,
    rotation: *mut c_void,
    scale_factor: i32,
) -> i32 {
    guard(
        "hp2_rs_engine_params_get_frotator_stream",
        0,
        move || unsafe {
            let (Some(svc), Some(rotation)) = (
                svc.cast::<ParamsServices>().as_ref(),
                rotation.cast::<FRotator>().as_mut(),
            ) else {
                return 0;
            };
            params::get_frotator_stream(svc, stream.cast::<TChar>(), rotation, scale_factor)
        },
    )
}

/// `GetFROTATOR(const TCHAR* Stream, const TCHAR* Match, FRotator& Value, INT ScaleFactor)` forward.
///
/// # Safety
/// `svc` must point to a live `ParamsServices`. `stream` and `matchp` must be
/// NUL-terminated. `rotation` must point to a writable `FRotator`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hp2_rs_engine_params_get_frotator(
    svc: *const c_void,
    stream: *const c_void,
    matchp: *const c_void,
    rotation: *mut c_void,
    scale_factor: i32,
) -> i32 {
    guard("hp2_rs_engine_params_get_frotator", 0, move || unsafe {
        let (Some(svc), Some(rotation)) = (
            svc.cast::<ParamsServices>().as_ref(),
            rotation.cast::<FRotator>().as_mut(),
        ) else {
            return 0;
        };
        params::get_frotator(
            svc,
            stream.cast::<TChar>(),
            matchp.cast::<TChar>(),
            rotation,
            scale_factor,
        )
    })
}
