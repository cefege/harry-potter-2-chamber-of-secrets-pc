//! SDL2 window-system handles for real WGPU surfaces.
//!
//! The ABI mirrors below are checked against the pinned SDL 2.32.10
//! `SDL_syswm.h`; SDL remains linked by the application CMake target rather
//! than bundled into the Rust static library.

use core::ffi::{c_int, c_void};
use core::mem::{align_of, offset_of, size_of};
use core::num::NonZeroIsize;
use core::ptr::NonNull;
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
    XlibDisplayHandle, XlibWindowHandle, WindowsDisplayHandle, Win32WindowHandle,
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SdlVersion {
    major: u8,
    minor: u8,
    patch: u8,
}

#[repr(C, align(8))]
struct SdlSyswmUnion {
    bytes: [u8; 64],
}

#[repr(C)]
struct SdlSyswmInfo {
    version: SdlVersion,
    subsystem: c_int,
    info: SdlSyswmUnion,
}

#[repr(C)]
struct SdlX11Info {
    display: *mut c_void,
    window: usize,
}

#[repr(C)]
struct SdlWaylandInfo {
    display: *mut c_void,
    surface: *mut c_void,
    _tail: [*mut c_void; 6],
}

#[repr(C)]
struct SdlWin32Info {
    window: *mut c_void,
    _hdc: *mut c_void,
    _hinstance: *mut c_void,
}

const _: () = {
    assert!(size_of::<SdlSyswmUnion>() == 64);
    assert!(align_of::<SdlSyswmUnion>() == 8);
    assert!(size_of::<SdlSyswmInfo>() == 72);
    assert!(offset_of!(SdlSyswmInfo, subsystem) == 4);
    assert!(offset_of!(SdlSyswmInfo, info) == 8);
    assert!(size_of::<SdlX11Info>() == 16);
    assert!(size_of::<SdlWaylandInfo>() == 64);
    assert!(size_of::<SdlWin32Info>() == 24);
};

unsafe extern "C" {
    fn SDL_GetVersion(version: *mut SdlVersion);
    fn SDL_GetWindowWMInfo(window: *mut c_void, info: *mut SdlSyswmInfo) -> c_int;
}

/// SDL's `SDL_SYSWM_TYPE` values from the pinned `SDL_syswm.h`.
const SDL_SYSWM_WINDOWS: c_int = 1;
const SDL_SYSWM_X11: c_int = 2;
const SDL_SYSWM_WAYLAND: c_int = 6;

#[derive(Clone, Copy)]
pub struct SdlWindowHandles {
    display: RawDisplayHandle,
    window: RawWindowHandle,
}

/// Returns the platform's real SDL window/display handles, or `None` when SDL
/// cannot expose a supported native window-system pair.
///
/// # Safety
/// `window` must be a live SDL2 window on the calling thread.
pub unsafe fn window_handles(window: *mut c_void) -> Option<SdlWindowHandles> {
    if window.is_null() {
        return None;
    }
    let mut info = SdlSyswmInfo {
        version: SdlVersion::default(),
        subsystem: 0,
        info: SdlSyswmUnion { bytes: [0; 64] },
    };
    unsafe { SDL_GetVersion(&mut info.version) };
    if unsafe { SDL_GetWindowWMInfo(window, &mut info) } == 0 {
        return None;
    }
    match info.subsystem {
        SDL_SYSWM_X11 => {
            let x11 = unsafe { &*((&info.info as *const SdlSyswmUnion).cast::<SdlX11Info>()) };
            let display = NonNull::new(x11.display)?;
            if x11.window == 0 {
                return None;
            }
            Some(SdlWindowHandles {
                display: RawDisplayHandle::Xlib(XlibDisplayHandle::new(Some(display), 0)),
                window: RawWindowHandle::Xlib(XlibWindowHandle::new(x11.window as _)),
            })
        }
        SDL_SYSWM_WAYLAND => {
            let wayland = unsafe { &*((&info.info as *const SdlSyswmUnion).cast::<SdlWaylandInfo>()) };
            Some(SdlWindowHandles {
                display: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(NonNull::new(wayland.display)?)),
                window: RawWindowHandle::Wayland(WaylandWindowHandle::new(NonNull::new(wayland.surface)?)),
            })
        }
        SDL_SYSWM_WINDOWS => {
            let win32 = unsafe { &*((&info.info as *const SdlSyswmUnion).cast::<SdlWin32Info>()) };
            let hwnd = NonZeroIsize::new(win32.window as isize)?;
            Some(SdlWindowHandles {
                display: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                window: RawWindowHandle::Win32(unsafe { Win32WindowHandle::new(hwnd) }),
            })
        }
        _ => None,
    }
}

/// Builds a real WGPU surface from SDL's borrowed native handles.
///
/// # Safety
/// The window and its native display must stay live on the platform's required
/// thread until the returned surface has been dropped.
pub unsafe fn create_surface_from_handles(
    instance: &wgpu::Instance,
    handles: SdlWindowHandles,
) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
    unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(handles.display),
            raw_window_handle: handles.window,
        })
    }
}
