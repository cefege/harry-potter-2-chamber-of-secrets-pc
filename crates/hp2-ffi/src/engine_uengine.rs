//! `UEngine` and `UServerCommandlet::Main` exports (`crates/hp2-engine/src/uengine.rs`).
//! The `UEngine` methods in `Engine/Src/UnEngine.cpp` forward here. The engine
//! object crosses as `void*`; engine calls go through the `Hp2UEngineServices`
//! table passed on every call.

use std::ffi::c_void;

use hp2_engine::uengine::{self, Hp2UEngineServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2UEngineServices`.
unsafe fn svc_of<'a>(p: *const Hp2UEngineServices) -> Option<&'a Hp2UEngineServices> {
    unsafe { p.as_ref() }
}

/// `UEngine::InitAudio()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_init_audio(
    svc: *const Hp2UEngineServices,
    engine: *mut c_void,
) {
    guard("hp2_rs_engine_uengine_init_audio", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), engine.is_null()) {
            uengine::init_audio(s, engine);
        }
    })
}

/// `UEngine::Init()` after the `NAMES_ONLY` block.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_init(svc: *const Hp2UEngineServices, engine: *mut c_void) {
    guard("hp2_rs_engine_uengine_init", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), engine.is_null()) {
            uengine::init(s, engine);
        }
    })
}

/// `UEngine::Exit()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_exit(svc: *const Hp2UEngineServices, engine: *mut c_void) {
    guard("hp2_rs_engine_uengine_exit", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), engine.is_null()) {
            uengine::exit(s, engine);
        }
    })
}

/// `UEngine::Destroy()` before `Super::Destroy()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_destroy(
    svc: *const Hp2UEngineServices,
    engine: *mut c_void,
) {
    guard("hp2_rs_engine_uengine_destroy", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), engine.is_null()) {
            uengine::destroy(s, engine);
        }
    })
}

/// `UEngine::Flush(AllowPrecache)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_flush(
    svc: *const Hp2UEngineServices,
    engine: *mut c_void,
    allow_precache: i32,
) {
    guard("hp2_rs_engine_uengine_flush", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), engine.is_null()) {
            uengine::flush(s, engine, allow_precache);
        }
    })
}

/// `UEngine::GetMaxTickRate()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_get_max_tick_rate(svc: *const Hp2UEngineServices) -> f32 {
    guard(
        "hp2_rs_engine_uengine_get_max_tick_rate",
        0.0,
        move || unsafe {
            match svc_of(svc) {
                Some(s) => uengine::get_max_tick_rate(s),
                None => 0.0,
            }
        },
    )
}

/// `UEngine::Exec(Cmd, Ar)`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_exec(
    svc: *const Hp2UEngineServices,
    engine: *mut c_void,
    cmd: *const u32,
    ar: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_uengine_exec", 0, move || unsafe {
        match (svc_of(svc), engine.is_null()) {
            (Some(s), false) => uengine::exec(s, engine, cmd, ar),
            _ => 0,
        }
    })
}

/// `UEngine::Key(Viewport, Key)`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_key(
    svc: *const Hp2UEngineServices,
    viewport: *mut c_void,
    key: i32,
) -> i32 {
    guard("hp2_rs_engine_uengine_key", 0, move || unsafe {
        match (svc_of(svc), viewport.is_null()) {
            (Some(s), false) => uengine::key(s, viewport, key),
            _ => 0,
        }
    })
}

/// `UEngine::InputEvent(Viewport, iKey, State, Delta)`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_input_event(
    svc: *const Hp2UEngineServices,
    viewport: *mut c_void,
    i_key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    guard("hp2_rs_engine_uengine_input_event", 0, move || unsafe {
        match (svc_of(svc), viewport.is_null()) {
            (Some(s), false) => uengine::input_event(s, viewport, i_key, state, delta),
            _ => 0,
        }
    })
}

/// `UServerCommandlet::Main(Parms)`. Returns `INT`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_uengine_commandlet_main(svc: *const Hp2UEngineServices) -> i32 {
    guard("hp2_rs_engine_uengine_commandlet_main", 0, move || unsafe {
        match svc_of(svc) {
            Some(s) => uengine::commandlet_main(s),
            None => 0,
        }
    })
}
