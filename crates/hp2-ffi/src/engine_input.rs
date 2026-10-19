//! `UInput` exports (`crates/hp2-engine/src/input.rs`). The `UInput` methods in
//! `Engine/Src/UnIn.cpp` forward here. Engine objects cross as `void*` and are
//! cast to the generated `UInput` mirror; engine calls go through the
//! `Hp2InputServices` table passed on every call.

use std::ffi::c_void;
use std::ptr;

use hp2_abi::engine::generated::UInput;
use hp2_engine::input::{self, Hp2InputServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2InputServices`.
unsafe fn svc_of<'a>(p: *const Hp2InputServices) -> Option<&'a Hp2InputServices> {
    unsafe { p.as_ref() }
}

/// Casts the opaque `UInput*` the C++ shell passes.
fn uinput(obj: *mut c_void) -> *mut UInput {
    obj.cast()
}

/// `UInput::Init(UViewport*)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_init(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    viewport: *mut c_void,
) {
    guard("hp2_rs_engine_input_init", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), obj.is_null()) {
            input::init(s, uinput(obj), viewport);
        }
    })
}

/// `UInput::Exec(const TCHAR*, FOutputDevice&)`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_exec(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    cmd: *const u32,
    ar: *mut c_void,
) -> i32 {
    guard("hp2_rs_engine_input_exec", 0, move || unsafe {
        match (svc_of(svc), obj.is_null()) {
            (Some(s), false) => input::exec(s, uinput(obj), cmd, ar),
            _ => 0,
        }
    })
}

/// `UInput::ExecInputCommands(const TCHAR*, FOutputDevice&)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_exec_input_commands(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    cmd: *const u32,
    ar: *mut c_void,
) {
    guard(
        "hp2_rs_engine_input_exec_input_commands",
        (),
        move || unsafe {
            if let (Some(s), false) = (svc_of(svc), obj.is_null()) {
                input::exec_input_commands(s, uinput(obj), cmd, ar);
            }
        },
    )
}

/// `UInput::FindButtonName(const TCHAR*, AActor*)`. Returns `BYTE*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_find_button_name(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    actor: *mut c_void,
    name: *const u32,
) -> *mut u8 {
    guard(
        "hp2_rs_engine_input_find_button_name",
        ptr::null_mut(),
        move || unsafe {
            match (svc_of(svc), obj.is_null()) {
                (Some(s), false) => input::find_button_name(s, uinput(obj), actor, name),
                _ => ptr::null_mut(),
            }
        },
    )
}

/// `UInput::FindAxisName(const TCHAR*, AActor*)`. Returns `FLOAT*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_find_axis_name(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    actor: *mut c_void,
    name: *const u32,
) -> *mut f32 {
    guard(
        "hp2_rs_engine_input_find_axis_name",
        ptr::null_mut(),
        move || unsafe {
            match (svc_of(svc), obj.is_null()) {
                (Some(s), false) => input::find_axis_name(s, uinput(obj), actor, name),
                _ => ptr::null_mut(),
            }
        },
    )
}

/// `UInput::PreProcess(EInputKey, EInputAction)`. Returns `UBOOL`. Uses no services.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_pre_process(obj: *mut c_void, key: i32, state: i32) -> i32 {
    guard("hp2_rs_engine_input_pre_process", 0, move || unsafe {
        if obj.is_null() {
            0
        } else {
            input::pre_process(uinput(obj), key, state)
        }
    })
}

/// `UInput::Process(EInputKey, EInputAction, FLOAT, FOutputDevice&)`. Returns `UBOOL`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn hp2_rs_engine_input_process(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    ar: *mut c_void,
    key: i32,
    state: i32,
    delta: f32,
) -> i32 {
    guard("hp2_rs_engine_input_process", 0, move || unsafe {
        match (svc_of(svc), obj.is_null()) {
            (Some(s), false) => input::process(s, uinput(obj), ar, key, state, delta),
            _ => 0,
        }
    })
}

/// `UInput::DirectAxis(EInputKey, FLOAT, FLOAT)`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_direct_axis(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    key: i32,
    length: f32,
    delta: f32,
) {
    guard("hp2_rs_engine_input_direct_axis", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), obj.is_null()) {
            input::direct_axis(s, uinput(obj), key, length, delta);
        }
    })
}

/// `UInput::ReadInput(FLOAT, FOutputDevice&)`. The C++ never uses `Ar`, so
/// the export does not take it.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_read_input(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    delta_seconds: f32,
) {
    guard("hp2_rs_engine_input_read_input", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), obj.is_null()) {
            input::read_input(s, uinput(obj), delta_seconds);
        }
    })
}

/// `UInput::ResetInput()`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_reset_input(svc: *const Hp2InputServices, obj: *mut c_void) {
    guard("hp2_rs_engine_input_reset_input", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), obj.is_null()) {
            input::reset_input(s, uinput(obj));
        }
    })
}

/// `UInput::GetKeyName(EInputKey)`. Returns a `const TCHAR*`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_get_key_name(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    key: i32,
) -> *const u32 {
    guard(
        "hp2_rs_engine_input_get_key_name",
        ptr::null(),
        move || unsafe {
            match (svc_of(svc), obj.is_null()) {
                (Some(s), false) => input::get_key_name(s, uinput(obj), key),
                _ => ptr::null(),
            }
        },
    )
}

/// `UInput::FindKeyName(const TCHAR*, EInputKey&)`. Returns `UBOOL` and writes
/// the key through `i_key`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_find_key_name(
    svc: *const Hp2InputServices,
    obj: *mut c_void,
    key_name: *const u32,
    i_key: *mut i32,
) -> i32 {
    guard("hp2_rs_engine_input_find_key_name", 0, move || unsafe {
        match (svc_of(svc), obj.is_null() || i_key.is_null()) {
            (Some(s), false) => input::find_key_name(s, uinput(obj), key_name, i_key),
            _ => 0,
        }
    })
}

/// `AActor::execGetCurrentKeyState` body. Returns the `UBOOL` the C++ shell
/// copies into `Result`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_input_current_key_state(
    svc: *const Hp2InputServices,
    actor: *mut c_void,
    key: u8,
) -> i32 {
    guard("hp2_rs_engine_input_current_key_state", 0, move || unsafe {
        match svc_of(svc) {
            Some(s) if !actor.is_null() => input::current_key_state(s, actor, key),
            _ => 0,
        }
    })
}
