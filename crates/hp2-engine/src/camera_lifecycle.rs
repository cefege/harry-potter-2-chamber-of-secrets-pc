//! Port of `UViewport` construction and destruction sequencing from `UnCamera.cpp`.
//!
//! Native service leaves retain configured-class construction, virtual `CloseWindow`
//! dispatch, and platform-owned deletion. Rust owns the observable ordering and the
//! publication of newly constructed Canvas/Input pointers to the viewport ABI record.

use std::ffi::c_void;

use hp2_abi::engine::generated::{URenderDevice, UViewport};

use crate::camera::{CameraServices, checked_unit, checked_value};

type Res<T> = Result<T, i32>;

/// Execute `UViewport::UViewport` after the C++ object base has been constructed.
///
/// The registration precedes class construction, Canvas precedes Input, and both are
/// published before the initial timestamp, matching the native constructor's order.
///
/// # Safety
/// `viewport` must be a live, uniquely mutable `UViewport` whose outer `UClient` is
/// live. `svc` must be the matching native service table.
pub unsafe fn construct(svc: &CameraServices, viewport: *mut UViewport) -> Res<()> {
    unsafe {
        checked_unit!((svc.viewport_register)(viewport.cast()));
    }

    let canvas = unsafe { checked_value!((svc.viewport_create_canvas)(viewport.cast())) };
    unsafe { (*viewport).canvas = canvas };

    let input = unsafe { checked_value!((svc.viewport_create_input)()) };
    unsafe { (*viewport).input = input };

    unsafe {
        checked_unit!((svc.viewport_stamp_last_update_time)(viewport.cast()));
    }
    Ok(())
}

/// Execute `UViewport::Destroy` before the C++ shell invokes `Super::Destroy()`.
///
/// # Safety
/// `viewport` must be a live `UViewport` whose outer client and registered engine
/// objects remain valid for the entire call. `svc` must be the matching native service
/// table.
pub unsafe fn destroy(svc: &CameraServices, viewport: *mut UViewport) -> Res<()> {
    unsafe {
        checked_unit!((svc.viewport_detach_audio)(viewport.cast()));
    }
    unsafe {
        checked_unit!((svc.viewport_close_window)(viewport.cast()));
    }
    unsafe {
        checked_unit!((svc.viewport_unregister)(viewport.cast()));
    }
    Ok(())
}

/// Execute the non-reflection portion of `URenderDevice::StaticConstructor`.
///
/// # Safety
/// `render_dev` must be a live `URenderDevice` while the matching native service
/// table is live.
pub unsafe fn set_render_device_defaults(_svc: &CameraServices, render_dev: *mut c_void) -> Res<()> {
    unsafe {
        (*render_dev.cast::<URenderDevice>()).decomp_format = crate::tex::TEXF_P8;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::mem::{size_of, transmute, zeroed};

    use super::*;

    thread_local! {
        static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn unexpected_service() {
        panic!("unexpected camera service call")
    }

    fn emit(event: &'static str) {
        EVENTS.with(|events| events.borrow_mut().push(event));
    }

    unsafe extern "C" fn register(_viewport: *mut c_void) -> i32 {
        emit("register");
        0
    }
    unsafe extern "C" fn canvas(_viewport: *mut c_void, out: *mut *mut c_void) -> i32 {
        emit("canvas");
        unsafe { out.write(0xCAFEusize as *mut c_void) };
        0
    }
    unsafe extern "C" fn canvas_failure(_viewport: *mut c_void, _out: *mut *mut c_void) -> i32 {
        emit("canvas");
        7
    }
    unsafe extern "C" fn input(out: *mut *mut c_void) -> i32 {
        emit("input");
        unsafe { out.write(0xBEEFusize as *mut c_void) };
        0
    }
    unsafe extern "C" fn stamp(_viewport: *mut c_void) -> i32 {
        emit("stamp");
        0
    }
    unsafe extern "C" fn detach(_viewport: *mut c_void) -> i32 {
        emit("detach");
        0
    }
    unsafe extern "C" fn close(_viewport: *mut c_void) -> i32 {
        emit("close");
        0
    }
    unsafe extern "C" fn unregister(_viewport: *mut c_void) -> i32 {
        emit("unregister");
        0
    }

    fn services(
        canvas_service: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    ) -> CameraServices {
        const WORDS: usize = size_of::<CameraServices>() / size_of::<usize>();
        let filler = unexpected_service as unsafe extern "C" fn() as usize;
        let table: [usize; WORDS] = [filler; WORDS];
        // SAFETY: CameraServices contains only function pointers. Every invoked entry is
        // replaced below; all other entries are a deliberate fail-fast test sentinel.
        let mut svc: CameraServices = unsafe { transmute(table) };
        svc.viewport_register = register;
        svc.viewport_create_canvas = canvas_service;
        svc.viewport_create_input = input;
        svc.viewport_stamp_last_update_time = stamp;
        svc.viewport_detach_audio = detach;
        svc.viewport_close_window = close;
        svc.viewport_unregister = unregister;
        svc
    }

    fn take_events() -> Vec<&'static str> {
        EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
    }

    #[test]
    fn lifecycle_preserves_native_publication_and_teardown_order() {
        let svc = services(canvas);
        // SAFETY: zero is a valid pre-construction harness state for the ABI mirror.
        let mut viewport: UViewport = unsafe { zeroed() };

        unsafe { construct(&svc, &mut viewport) }.unwrap();
        assert_eq!(take_events(), ["register", "canvas", "input", "stamp"]);
        assert_eq!(viewport.canvas, 0xCAFEusize as *mut c_void);
        assert_eq!(viewport.input, 0xBEEFusize as *mut c_void);
        // SAFETY: zeroed URenderDevice is valid harness state for the ABI mirror.
        let mut render_dev: URenderDevice = unsafe { zeroed() };
        render_dev.decomp_format = 0xFF;
        unsafe { set_render_device_defaults(&svc, (&mut render_dev as *mut URenderDevice).cast()) }
            .unwrap();
        assert_eq!(render_dev.decomp_format, crate::tex::TEXF_P8);

        unsafe { destroy(&svc, &mut viewport) }.unwrap();
        assert_eq!(take_events(), ["detach", "close", "unregister"]);
    }

    #[test]
    fn construction_stops_before_pointer_publication_after_native_failure() {
        let svc = services(canvas_failure);
        // SAFETY: zero is a valid pre-construction harness state for the ABI mirror.
        let mut viewport: UViewport = unsafe { zeroed() };

        assert_eq!(unsafe { construct(&svc, &mut viewport) }, Err(-7));
        assert_eq!(take_events(), ["register", "canvas"]);
        assert!(viewport.canvas.is_null());
        assert!(viewport.input.is_null());
    }
}
