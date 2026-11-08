//! Replaces `ShowSplash`/`HideSplash` (`SDLLaunch.cpp`), minus the engine
//! file load (`appLoadFileToArray`, which stays C++: the engine's
//! abstracted file load, not a plain SDL/libc call). Once the BMP bytes
//! are in memory, this owns every SDL call: loading the surface, creating
//! the window (reusing [`crate::window_lifecycle::create_window`], same
//! `"Harry Potter 2"` title as the real game window), renderer, texture,
//! and the one-shot clear/copy/present, plus the paired teardown.
//!
//! All four SDL objects cross as a single opaque [`SplashHandle`]; same
//! design as `window_lifecycle`'s own `SdlWindow` -- the real C++ only
//! stores and passes the handle back, never reads through it.
//!
//! `ShowSplash`'s own invariant carries over exactly: on any failure, this
//! tears down whatever it already created and returns an all-null handle
//! (matching the original's `HideSplash(); return;` on every failure
//! branch), never a partially populated one.

#[cfg(feature = "sdl")]
use crate::window_lifecycle::{self, SdlWindow};
#[cfg(feature = "sdl")]
mod ffi {
    use super::SdlWindow;
    use std::ffi::c_void;
    use std::os::raw::c_int;

    #[repr(C)]
    pub struct SdlRwops {
        _private: [u8; 0],
    }

    /// Only the leading fields of the real (non-opaque) `SDL_Surface`
    /// (`SDL_surface.h`) are declared: this module never constructs or
    /// owns one by value, only reads `w`/`h` through a pointer SDL itself
    /// returned, so the true struct's larger real size is irrelevant.
    #[repr(C)]
    pub struct SdlSurface {
        pub flags: u32,
        pub format: *mut c_void,
        pub w: c_int,
        pub h: c_int,
    }

    #[repr(C)]
    pub struct SdlRenderer {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct SdlTexture {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn SDL_RWFromConstMem(mem: *const c_void, size: c_int) -> *mut SdlRwops;
        pub fn SDL_LoadBMP_RW(src: *mut SdlRwops, freesrc: c_int) -> *mut SdlSurface;
        pub fn SDL_FreeSurface(surface: *mut SdlSurface);
        pub fn SDL_CreateRenderer(
            window: *mut SdlWindow,
            index: c_int,
            flags: u32,
        ) -> *mut SdlRenderer;
        pub fn SDL_DestroyRenderer(renderer: *mut SdlRenderer);
        pub fn SDL_CreateTextureFromSurface(
            renderer: *mut SdlRenderer,
            surface: *mut SdlSurface,
        ) -> *mut SdlTexture;
        pub fn SDL_DestroyTexture(texture: *mut SdlTexture);
        pub fn SDL_RenderClear(renderer: *mut SdlRenderer) -> c_int;
        pub fn SDL_RenderCopy(
            renderer: *mut SdlRenderer,
            texture: *mut SdlTexture,
            srcrect: *const c_void,
            dstrect: *const c_void,
        ) -> c_int;
        pub fn SDL_RenderPresent(renderer: *mut SdlRenderer);
    }
}

#[cfg(feature = "sdl")]
pub use ffi::{SdlRenderer, SdlSurface, SdlTexture};

/// All four SDL objects `ShowSplash` creates together and `HideSplash`
/// destroys together. Every field is null together (not yet shown, or
/// `show_splash` failed) or non-null together (shown) -- never a mix.
#[cfg(feature = "sdl")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SplashHandle {
    pub window: *mut SdlWindow,
    pub renderer: *mut SdlRenderer,
    pub texture: *mut SdlTexture,
    pub surface: *mut SdlSurface,
}

#[cfg(feature = "sdl")]
impl SplashHandle {
    const NULL: Self = Self {
        window: std::ptr::null_mut(),
        renderer: std::ptr::null_mut(),
        texture: std::ptr::null_mut(),
        surface: std::ptr::null_mut(),
    };
}

/// Replaces `ShowSplash`'s SDL-side body (`SDLLaunch.cpp:613-648`), taking
/// the already-loaded BMP bytes. Returns an all-null handle on any
/// failure, tearing down whatever it already created -- see the module
/// doc's invariant. Ignores every SDL render call's result, matching the
/// original.
#[cfg(feature = "sdl")]
pub fn show_splash(bmp_bytes: &[u8]) -> SplashHandle {
    use std::os::raw::c_int;

    let Ok(len) = c_int::try_from(bmp_bytes.len()) else {
        return SplashHandle::NULL;
    };
    // SAFETY: `bmp_bytes` is a valid slice for the duration of this call;
    // `SDL_RWFromConstMem` only reads through it during `SDL_LoadBMP_RW`,
    // which runs before this function returns.
    let stream = unsafe { ffi::SDL_RWFromConstMem(bmp_bytes.as_ptr().cast(), len) };
    if stream.is_null() {
        return SplashHandle::NULL;
    }
    // SAFETY: `stream` was just constructed above; `freesrc = 1` matches
    // the original, which lets SDL close the RWops itself.
    let surface = unsafe { ffi::SDL_LoadBMP_RW(stream, 1) };
    if surface.is_null() {
        return SplashHandle::NULL;
    }
    // SAFETY: `surface` is non-null and was just returned by SDL above.
    let (w, h) = unsafe { ((*surface).w, (*surface).h) };

    let window = window_lifecycle::create_window(w, h, 0);
    if window.is_null() {
        unsafe { ffi::SDL_FreeSurface(surface) };
        return SplashHandle::NULL;
    }

    // SAFETY: `window` is non-null and was just created above.
    let renderer = unsafe { ffi::SDL_CreateRenderer(window, -1, 0) };
    // SAFETY: `renderer`, if non-null, was just created above; `surface`
    // is non-null and still valid.
    let texture = if renderer.is_null() {
        std::ptr::null_mut()
    } else {
        unsafe { ffi::SDL_CreateTextureFromSurface(renderer, surface) }
    };
    if renderer.is_null() || texture.is_null() {
        // SAFETY: each pointer passed below is either null (skipped) or
        // was validly created above, for this one teardown.
        unsafe {
            if !texture.is_null() {
                ffi::SDL_DestroyTexture(texture);
            }
            if !renderer.is_null() {
                ffi::SDL_DestroyRenderer(renderer);
            }
            ffi::SDL_FreeSurface(surface);
            window_lifecycle::destroy_window(window);
        }
        return SplashHandle::NULL;
    }

    // SAFETY: `renderer`/`texture` are both non-null and valid here.
    unsafe {
        ffi::SDL_RenderClear(renderer);
        ffi::SDL_RenderCopy(renderer, texture, std::ptr::null(), std::ptr::null());
        ffi::SDL_RenderPresent(renderer);
    }

    SplashHandle {
        window,
        renderer,
        texture,
        surface,
    }
}

/// Replaces `HideSplash` (`SDLLaunch.cpp:587-609`): destroys whichever of
/// the four fields are non-null (independently, exactly matching the
/// original's four separate null checks) and returns an all-null handle
/// for the caller to store back.
///
/// # Safety
/// Each non-null field of `handle` must be the exact pointer `show_splash`
/// returned (or null), not yet destroyed, and not used again afterward.
#[cfg(feature = "sdl")]
pub unsafe fn hide_splash(handle: SplashHandle) -> SplashHandle {
    unsafe {
        if !handle.texture.is_null() {
            ffi::SDL_DestroyTexture(handle.texture);
        }
        if !handle.renderer.is_null() {
            ffi::SDL_DestroyRenderer(handle.renderer);
        }
        if !handle.surface.is_null() {
            ffi::SDL_FreeSurface(handle.surface);
        }
        if !handle.window.is_null() {
            window_lifecycle::destroy_window(handle.window);
        }
    }
    SplashHandle::NULL
}
