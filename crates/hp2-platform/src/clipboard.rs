//! System clipboard access, replacing
//! `USDLClient::GetClipboardText`/`SetClipboardText`
//! (`SDLDrv/Src/SDLClient.cpp:279-304`).
//!
//! **Design: hand-written `extern "C"` declarations against the pinned
//! SDL2 headers, no `sdl2`/`sdl2-sys` crate dependency at all, behind
//! the `sdl` Cargo feature -- the same pattern already in use for
//! `hp2-import`'s libarchive binding
//! (`crates/hp2-import/src/archive.rs`'s `archive` feature; see this
//! crate's "Approved deviations" entry in
//! `local://rust-inplace-port-plan.md` for the full history).** A
//! crate dependency on `sdl2`/`sdl2-sys` with the `static-link`
//! feature does not work here: building `hp2-ffi` as a
//! `crate-type = ["staticlib"]` bundles the *entire* referenced
//! `SDL2`/`SDL2main` static archives' object code directly into
//! `libhp2_ffi.a` (rustc's default behavior for `-l static=X`
//! references in a staticlib output), and `hp2_game` already links
//! `SDL2::SDL2-static` on its own account (`SDLDrv`, `SDLLaunch`,
//! `HP2LauncherPlatform.cpp`) -- a second, separately-embedded copy of
//! the same archive's object code in the same binary.
//!
//! Hand-written externs with no `#[link(...)]` attribute and no
//! build-script-emitted `cargo:rustc-link-lib` sidestep this
//! entirely: nothing in this crate's own compiled output records a
//! native-lib dependency for rustc/Corrosion to eagerly resolve or
//! bundle. The *only* place `SDL2::SDL2-static` is ever linked is
//! `hp2_ffi`'s CMake `INTERFACE` (`target_link_libraries(hp2_ffi
//! INTERFACE SDL2::SDL2-static)`, matching `archive_static`'s own
//! entry immediately above it in `HP2Targets.cmake`) -- the *same*
//! archive every other SDL2 consumer in this build already links,
//! linked exactly once into `hp2_game`'s final binary. Verified:
//! `nm out/<preset>/libhp2_ffi.a | grep ' T _SDL_Init$'` is empty
//! (no embedded definition).
//!
//! **No SDL init in this module, deliberately:** by the time any
//! caller could reach these functions, C++ has already called
//! `SDL_Init(SDL_INIT_VIDEO)` on the process main thread
//! (`USDLClient::Init`), so these are thin, stateless calls on
//! already-initialized SDL state, not reinitializing anything.
//! `SDL_InitSubSystem(VIDEO)` touches Cocoa/AppKit APIs that macOS
//! requires to run on the process's actual main thread -- the Rust
//! test harness runs each test on a worker thread, never thread 0, so
//! this module's functions cannot be exercised by a normal in-crate
//! `#[test]` (confirmed: calling `sdl2::init()`/`.video()` from a
//! `#[test]` aborts with "Rust cannot catch foreign exceptions";
//! calling the same sequence from a real `fn main()` on the actual
//! main thread works cleanly).
//!
//! Verified by running the compiled `hp2_rs_platform_get_clipboard_text`/
//! `set_clipboard_text` functions directly from C++
//! (`Tests/PlatformClipboardTests.cpp`'s `clipboard_round_trip` ctest,
//! main-thread SDL init included, same as the real engine) -- not a
//! constructed `USDLClient` instance calling through
//! `GetClipboardText`/`SetClipboardText` (that wrapper is a three-line
//! pass-through plus UE1's own already-covered TCHAR/UTF-8 conversion,
//! not itself at risk), and not a Rust-level `#[test]` either --
//! matching `archive.rs`'s own verification-by-external-probe approach
//! for the same structural reason (the gated code needs real linkage
//! against a pinned native library that a bare `cargo test` build
//! doesn't have).

#[cfg(feature = "sdl")]
use std::os::raw::{c_char, c_int, c_void};

#[cfg(feature = "sdl")]
unsafe extern "C" {
    // SDL2 2.32.10 (`SDL_clipboard.h`); hand-written against the pinned
    // headers (`out/<preset>/_deps/hp2_sdl2-src/include/SDL_clipboard.h`),
    // not generated -- three stable, long-unchanged signatures.
    fn SDL_GetClipboardText() -> *mut c_char;
    fn SDL_SetClipboardText(text: *const c_char) -> c_int;
    fn SDL_free(mem: *mut c_void);
}

/// `USDLClient::GetClipboardText` (`SDLClient.cpp:279-294`). Returns the
/// clipboard contents as UTF-8, or `None` when the clipboard is empty,
/// unavailable, or not valid UTF-8 -- the real C++ returns an empty
/// `FString` in the first two cases (`SDL_GetClipboardText` returning
/// `NULL`/empty) and would itself produce garbled `TCHAR`s on the third,
/// which `None` maps to the same "nothing usable" outcome.
#[cfg(feature = "sdl")]
pub fn get_clipboard_text() -> Option<String> {
    // SAFETY: `SDL_GetClipboardText` is safe to call at any time once SDL's
    // video subsystem is initialized (true by construction: C++ owns that
    // init and only calls into this crate afterward); it always returns a
    // valid NUL-terminated pointer (never NULL per SDL2's own doc, "empty
    // string on failure"), which must be freed with `SDL_free`.
    unsafe {
        let raw = SDL_GetClipboardText();
        if raw.is_null() {
            return None;
        }
        let text = std::ffi::CStr::from_ptr(raw)
            .to_str()
            .ok()
            .map(str::to_owned);
        SDL_free(raw as *mut c_void);
        text.filter(|s| !s.is_empty())
    }
}

/// `USDLClient::SetClipboardText` (`SDLClient.cpp:296-304`). Returns
/// whether `SDL_SetClipboardText` succeeded, matching the real C++'s
/// `UBOOL` return. `false` for text containing an embedded NUL (cannot be
/// represented as a C string; the real C++'s `TCHAR*` -> UTF-8 conversion
/// has no such input today, so this is unreachable in practice, not a
/// behavior divergence).
#[cfg(feature = "sdl")]
pub fn set_clipboard_text(text: &str) -> bool {
    let Ok(c_text) = std::ffi::CString::new(text) else {
        return false;
    };
    // SAFETY: `c_text` is a valid NUL-terminated C string for the duration
    // of this call; `SDL_SetClipboardText` copies it internally.
    unsafe { SDL_SetClipboardText(c_text.as_ptr()) == 0 }
}
