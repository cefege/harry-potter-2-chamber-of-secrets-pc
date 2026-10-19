//! Console exports (`crates/hp2-engine/src/console.rs`). `UConsole` methods in
//! `Engine/Src/UnCon.cpp` forward here. Engine objects cross as `void*`;
//! engine calls go through the `Hp2ConsoleServices` table passed on every call.

use std::ffi::c_void;

use hp2_abi::engine::generated::{FSceneNode, UConsole};
use hp2_engine::console::{self, Hp2ConsoleServices};
use hp2_ffi_support::guard;

/// Borrows the services table; null is `None`.
///
/// # Safety
/// `p`, when non-null, must point to a live `Hp2ConsoleServices`.
unsafe fn svc_of<'a>(p: *const Hp2ConsoleServices) -> Option<&'a Hp2ConsoleServices> {
    unsafe { p.as_ref() }
}

/// `UConsole::_Init` after `VERIFY_CLASS_SIZE`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_init(
    svc: *const Hp2ConsoleServices,
    console: *mut c_void,
    viewport: *mut c_void,
) {
    guard("hp2_rs_engine_console_init", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), console.is_null()) {
            console::init(s, console.cast::<UConsole>(), viewport);
        }
    })
}

/// `UConsole::Serialize`: forwards to `eventMessage`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_serialize(
    svc: *const Hp2ConsoleServices,
    console: *mut c_void,
    data: *const c_void,
    name: i32,
) {
    guard("hp2_rs_engine_console_serialize", (), move || unsafe {
        if let (Some(s), false) = (svc_of(svc), console.is_null()) {
            console::serialize(s, console.cast::<UConsole>(), data, name);
        }
    })
}

/// `UConsole::execConsoleCommand` body. Returns the `UBOOL` for `Result`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_exec_command(
    svc: *const Hp2ConsoleServices,
    console: *mut c_void,
    cmd: *const c_void,
) -> i32 {
    guard("hp2_rs_engine_console_exec_command", 0, move || unsafe {
        match (svc_of(svc), console.is_null()) {
            (Some(s), false) => console::exec_command(s, console.cast::<UConsole>(), cmd),
            _ => 0,
        }
    })
}

/// `UConsole::execSaveTimeDemo` body.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_save_time_demo(
    svc: *const Hp2ConsoleServices,
    string: *const c_void,
) {
    guard("hp2_rs_engine_console_save_time_demo", (), move || unsafe {
        if let Some(s) = svc_of(svc) {
            console::save_time_demo(s, string);
        }
    })
}

/// `UConsole::execCreateNativeFont` body. Returns `UFont*` for `Result`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_create_native_font(
    svc: *const Hp2ConsoleServices,
    console: *mut c_void,
    name: *const c_void,
    height: i32,
) -> *mut c_void {
    guard(
        "hp2_rs_engine_console_create_native_font",
        std::ptr::null_mut(),
        move || unsafe {
            match (svc_of(svc), console.is_null()) {
                (Some(s), false) => {
                    console::create_native_font(s, console.cast::<UConsole>(), name, height)
                }
                _ => std::ptr::null_mut(),
            }
        },
    )
}

/// `UConsole::GetDrawWorld`: `!bNoDrawWorld`. Returns `UBOOL`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_get_draw_world(console: *const c_void) -> i32 {
    guard("hp2_rs_engine_console_get_draw_world", 0, move || unsafe {
        if console.is_null() {
            0
        } else {
            console::get_draw_world(console.cast::<UConsole>())
        }
    })
}

/// `UConsole::PreRender`.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_pre_render(
    svc: *const Hp2ConsoleServices,
    console: *mut c_void,
    frame: *mut c_void,
) {
    guard("hp2_rs_engine_console_pre_render", (), move || unsafe {
        if let (Some(s), false, false) = (svc_of(svc), console.is_null(), frame.is_null()) {
            console::pre_render(s, console.cast::<UConsole>(), frame.cast::<FSceneNode>());
        }
    })
}

/// `UConsole::PostRender`. Needs no services: it restores the saved frame.
#[unsafe(no_mangle)]
pub extern "C" fn hp2_rs_engine_console_post_render(console: *mut c_void, frame: *mut c_void) {
    guard("hp2_rs_engine_console_post_render", (), move || unsafe {
        if !console.is_null() && !frame.is_null() {
            console::post_render(console.cast::<UConsole>(), frame.cast::<FSceneNode>());
        }
    })
}
