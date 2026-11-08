//! Replaces `USDLViewport`'s SDL system cursor create/free/set leaves
//! (`SDLViewport.cpp:224-235`). `Hp2PlatformViewportLeaf_CursorStore` (a
//! pure field write, no SDL call) is the only cursor leaf the C++ shell
//! still performs; it publishes the pointers this module's
//! `create_system_cursor` returns and clears them after `free_cursor`.
//!
//! `SdlCursor` crosses as an opaque pointer, same design as
//! `window_lifecycle::SdlWindow`: the real C++ owns the
//! `SystemCursors[NumSystemCursors]` array it is stored in; this module
//! only creates, destroys, or applies it on request.

#[cfg(feature = "sdl")]
mod ffi {
    /// Opaque; every caller (including the real C++) only ever holds a
    /// pointer to this, never reads its layout.
    #[repr(C)]
    pub struct SdlCursor {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn SDL_CreateSystemCursor(id: std::os::raw::c_int) -> *mut SdlCursor;
        pub fn SDL_FreeCursor(cursor: *mut SdlCursor);
        pub fn SDL_SetCursor(cursor: *mut SdlCursor);
    }
}

#[cfg(feature = "sdl")]
pub use ffi::SdlCursor;

/// `SDL_SystemCursor` (`SDL_mouse.h`), only the entries
/// `Hp2PlatformViewportLeaf_CursorCreate`'s table used, in `SDLDrv.h`'s
/// `NumSystemCursors` slot order: arrow, sizeall, sizenesw, sizens,
/// sizenwse, sizewe, wait (`SDLViewport.cpp:226-228`).
#[cfg(feature = "sdl")]
const SYSTEM_CURSORS: [std::os::raw::c_int; 7] = [
    0, // SDL_SYSTEM_CURSOR_ARROW
    9, // SDL_SYSTEM_CURSOR_SIZEALL
    6, // SDL_SYSTEM_CURSOR_SIZENESW
    8, // SDL_SYSTEM_CURSOR_SIZENS
    5, // SDL_SYSTEM_CURSOR_SIZENWSE
    7, // SDL_SYSTEM_CURSOR_SIZEWE
    2, // SDL_SYSTEM_CURSOR_WAIT
];

/// Replaces `Hp2PlatformViewportLeaf_CursorCreate` (`SDLViewport.cpp:
/// 224-231`): `SDL_CreateSystemCursor(Cursors[slot])`. `slot` is
/// `0..NumSystemCursors`, the same table index the real C++ used. The
/// caller stores the (possibly null, on failure) result itself via
/// `CursorStore`.
#[cfg(feature = "sdl")]
pub fn create_system_cursor(slot: i32) -> *mut SdlCursor {
    let id = SYSTEM_CURSORS[slot as usize];
    // SAFETY: SDL_CreateSystemCursor is safe to call once SDL's video
    // subsystem is initialized (true by construction: C++ owns that
    // init and only calls into this crate afterward); `id` is a valid
    // enum value from the fixed table above.
    unsafe { ffi::SDL_CreateSystemCursor(id) }
}

/// Replaces `Hp2PlatformViewportLeaf_CursorFree` (`SDLViewport.cpp:233`).
///
/// # Safety
/// `cursor` must be a valid, non-null `SDL_Cursor*` for the duration of
/// this call; the caller must not use it again afterward.
#[cfg(feature = "sdl")]
pub unsafe fn free_cursor(cursor: *mut SdlCursor) {
    // SAFETY: validity is this function's own contract (see above).
    unsafe { ffi::SDL_FreeCursor(cursor) }
}

/// Replaces `Hp2PlatformViewportLeaf_CursorSet` (`SDLViewport.cpp:235`).
///
/// # Safety
/// `cursor` must be a valid, non-null `SDL_Cursor*` for the duration of
/// this call.
#[cfg(feature = "sdl")]
pub unsafe fn set_cursor(cursor: *mut SdlCursor) {
    // SAFETY: validity is this function's own contract (see above).
    unsafe { ffi::SDL_SetCursor(cursor) }
}
