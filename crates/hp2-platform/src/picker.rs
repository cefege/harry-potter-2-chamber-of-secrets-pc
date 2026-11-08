//! Native folder/file picker dialogs, replacing
//! `HP2LauncherPlatform::PickFolder`/`PickFile`
//! (`SDLLaunch/Src/HP2LauncherPlatform.cpp:41-81`, deleted).
//!
//! **Design: hand-written `extern "C"` declarations against the pinned NFD
//! (Native File Dialog Extended) headers, no `rfd`/`nfd`/`nfd-sys` crate
//! dependency, behind the `nfd` Cargo feature -- the same pattern
//! `clipboard.rs` already established for SDL2 (see that module's doc
//! comment for the full staticlib-bundling rationale; identical reasoning
//! applies here: a crate-managed native link for NFD would duplicate the
//! exact same `nfd::nfd` archive `hp2_game` already needs to link on its
//! own account elsewhere in the build). The only place NFD is ever linked
//! is `hp2_ffi`'s CMake `INTERFACE` (`target_link_libraries(hp2_ffi
//! INTERFACE ... nfd::nfd)`), the same archive every other NFD consumer in
//! this build already links, resolved exactly once into `hp2_game`'s final
//! binary.
//!
//! Signatures are against the pinned header
//! (`out/macos-arm64/_deps/hp2_nativefiledialog-src/src/include/nfd.h`).
//!
//! Both pickers return the NFD result as raw bytes (`Vec<u8>`), never a
//! lossy UTF-8 `String`: the real C++ copied `selected` into `std::string`
//! verbatim via `Path.assign(selected)`, and on Linux NFD's `*U8`
//! functions alias the raw GTK byte result, so a non-UTF-8 filename must
//! survive the round trip unchanged, not come back as U+FFFD.
//!
//! `NFD_Init`/`NFD_PickFolderU8`/`NFD_OpenDialogU8` failures log through
//! `SDL_Log` with the original's exact format strings and arguments
//! (`HP2LauncherPlatform.cpp:26,53,72`), via a hand-written variadic
//! `extern "C"` binding behind the `sdl` feature (this crate's `nfd`
//! feature depends on `sdl`, matching the original TU always including
//! both `<SDL.h>` and `<nfd.h>`).

#[cfg(feature = "nfd")]
use std::ffi::{CStr, CString, c_char};

#[cfg(feature = "nfd")]
unsafe extern "C" {
    // SDL2 `SDL_log.h`; hand-written against the pinned header, matching
    // `sdl_runtime.rs`/`clipboard.rs`'s established pattern for this
    // crate's SDL2 FFI. Variadic, called only with a NUL-terminated
    // `*const c_char` for every `%s` and an `i32` for every `%d`, exactly
    // matching the original's own `SDL_Log(fmt, ...)` call shapes.
    fn SDL_Log(fmt: *const std::os::raw::c_char, ...);
}

#[cfg(feature = "nfd")]
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum NfdResult {
    Error = 0,
    Okay = 1,
    /// Never matched explicitly: the real C++ only ever branches on
    /// `NFD_OKAY` vs. `NFD_ERROR` (`HP2LauncherPlatform.cpp:47,53,66,72`),
    /// treating a cancelled dialog identically to any other non-okay,
    /// non-error outcome. Named for completeness against the pinned
    /// `nfdresult_t`.
    #[allow(dead_code)]
    Cancel = 2,
}

#[cfg(feature = "nfd")]
#[repr(C)]
struct NfdU8FilterItem {
    name: *const c_char,
    spec: *const c_char,
}

#[cfg(feature = "nfd")]
unsafe extern "C" {
    fn NFD_Init() -> NfdResult;
    fn NFD_Quit();
    fn NFD_PickFolderU8(out_path: *mut *mut c_char, default_path: *const c_char) -> NfdResult;
    fn NFD_OpenDialogU8(
        out_path: *mut *mut c_char,
        filter_list: *const NfdU8FilterItem,
        filter_count: u32,
        default_path: *const c_char,
    ) -> NfdResult;
    fn NFD_FreePathU8(file_path: *mut c_char);
    fn NFD_GetError() -> *const c_char;
}

/// `FNfdSession` (`HP2LauncherPlatform.cpp:21-32`): `NFD_Init`/`NFD_Quit`
/// RAII, quitting only if init actually succeeded.
#[cfg(feature = "nfd")]
struct NfdSession {
    ready: bool,
}

#[cfg(feature = "nfd")]
impl NfdSession {
    fn new() -> Self {
        // SAFETY: `NFD_Init` is safe to call at any time; this session's
        // `Drop` is the only place that calls `NFD_Quit`, and only when
        // this call actually succeeded.
        let ready = unsafe { NFD_Init() } == NfdResult::Okay;
        if !ready {
            // `if (!Ready) SDL_Log("HP2Launcher: native file dialog
            // initialization failed: %s", NFD_GetError());`
            // (`HP2LauncherPlatform.cpp:26`). `NFD_GetError()`'s pointer is
            // forwarded straight through, same as the original.
            // SAFETY: `NFD_GetError` always returns either null or a
            // NUL-terminated string owned by NFD, valid until the next NFD
            // call on this thread; `SDL_Log`'s format string has exactly
            // one `%s` for it.
            unsafe {
                SDL_Log(
                    c"HP2Launcher: native file dialog initialization failed: %s".as_ptr(),
                    NFD_GetError(),
                );
            }
        }
        Self { ready }
    }
}

#[cfg(feature = "nfd")]
impl Drop for NfdSession {
    fn drop(&mut self) {
        if self.ready {
            // SAFETY: paired with a successful `NFD_Init` above, called at
            // most once.
            unsafe { NFD_Quit() };
        }
    }
}

#[cfg(feature = "nfd")]
unsafe fn take_selected(selected: *mut c_char) -> Vec<u8> {
    // SAFETY: `selected` is a non-null NUL-terminated string NFD allocated
    // for this call, freed exactly once here via `NFD_FreePathU8`. Taken
    // as raw bytes, not validated/lossily-converted to UTF-8: the real
    // C++'s `Path.assign(selected)` is a byte-verbatim `std::string` copy.
    unsafe {
        let path = CStr::from_ptr(selected).to_bytes().to_vec();
        NFD_FreePathU8(selected);
        path
    }
}

/// `HP2LauncherPlatform::PickFolder` (`HP2LauncherPlatform.cpp:41-55`).
/// `None` on cancel, failed init, or dialog error (matching the real C++'s
/// untouched `Path` out-parameter on a `false` return). The returned bytes
/// are NFD's raw result, not validated UTF-8.
#[cfg(feature = "nfd")]
pub fn pick_folder() -> Option<Vec<u8>> {
    let session = NfdSession::new();
    if !session.ready {
        return None;
    }
    let mut selected: *mut c_char = std::ptr::null_mut();
    // SAFETY: `selected` is a valid out-pointer for the duration of this call.
    let result = unsafe { NFD_PickFolderU8(&mut selected, std::ptr::null()) };
    if result == NfdResult::Okay && !selected.is_null() {
        return Some(unsafe { take_selected(selected) });
    }
    if result == NfdResult::Error {
        // `if (result == NFD_ERROR) SDL_Log("HP2Launcher: folder picker
        // failed: %s", NFD_GetError());` (`HP2LauncherPlatform.cpp:53`).
        // SAFETY: same contract as the init-failure `SDL_Log` call above.
        unsafe {
            SDL_Log(
                c"HP2Launcher: folder picker failed: %s".as_ptr(),
                NFD_GetError(),
            );
        }
    }
    None
}

/// Builds the real C++'s `specification` string
/// (`HP2LauncherPlatform.cpp:59-64`): empty extensions are skipped, the
/// remainder comma-joined, exactly as the real C++'s loop over
/// `Extensions` built `specification`.
#[cfg(feature = "nfd")]
fn join_specification(extensions: &[&str]) -> String {
    let mut specification = String::new();
    for extension in extensions {
        if extension.is_empty() {
            continue;
        }
        if !specification.is_empty() {
            specification.push(',');
        }
        specification.push_str(extension);
    }
    specification
}

/// `HP2LauncherPlatform::PickFile` (`HP2LauncherPlatform.cpp:57-81`), called
/// only for the retail archive picker's nine extensions
/// (`crates/hp2-launcher/src/import_session.rs`'s `pick_archive`, via
/// `hp2_rs_launcher_pick_file`). `extensions` is joined into one NFD filter
/// spec by [`join_specification`], exactly as the real C++ built
/// `specification`; an empty result after joining picks with no filter at
/// all (`filterCount == 0`), matching the real C++'s
/// `specification.empty() ? nullptr : &filter` branch. `None` on cancel,
/// failed init, or dialog error (matching the real C++'s untouched `Path`
/// out-parameter on a `false` return). The returned bytes are NFD's raw
/// result, not validated UTF-8 (see [`pick_folder`]).
#[cfg(feature = "nfd")]
pub fn pick_file(extensions: &[&str]) -> Option<Vec<u8>> {
    let specification = join_specification(extensions);
    let Ok(name) = CString::new("Retail game archives") else {
        return None;
    };
    let Ok(spec) = CString::new(specification.as_str()) else {
        return None;
    };
    let filter = NfdU8FilterItem {
        name: name.as_ptr(),
        spec: spec.as_ptr(),
    };
    let session = NfdSession::new();
    if !session.ready {
        return None;
    }
    let mut selected: *mut c_char = std::ptr::null_mut();
    let (filter_list, filter_count) = if specification.is_empty() {
        (std::ptr::null(), 0u32)
    } else {
        (&filter as *const NfdU8FilterItem, 1u32)
    };
    // SAFETY: `selected` is a valid out-pointer; `filter_list`/`filter_count`
    // describe either no filter or one filter backed by `name`/`spec`, both
    // alive for the duration of this call.
    let result =
        unsafe { NFD_OpenDialogU8(&mut selected, filter_list, filter_count, std::ptr::null()) };
    if result == NfdResult::Okay && !selected.is_null() {
        return Some(unsafe { take_selected(selected) });
    }
    if result == NfdResult::Error {
        // `if (result == NFD_ERROR) SDL_Log("HP2Launcher: file picker
        // failed: %s", NFD_GetError());` (`HP2LauncherPlatform.cpp:72`).
        // SAFETY: same contract as the init-failure `SDL_Log` call above.
        unsafe {
            SDL_Log(
                c"HP2Launcher: file picker failed: %s".as_ptr(),
                NFD_GetError(),
            );
        }
    }
    None
}

#[cfg(all(test, feature = "nfd"))]
mod tests {
    use super::*;

    /// `specification` joining (`HP2LauncherPlatform.cpp:59-64`): empty
    /// extensions are skipped, the rest comma-joined in order. Exercises
    /// [`join_specification`] directly -- the exact function [`pick_file`]
    /// itself calls to build its filter spec, not a reimplementation of
    /// the original loop; the real dialog can't run headless in a unit
    /// test, but this is the real production code path up to the point
    /// where NFD takes over.
    #[test]
    fn join_specification_matches_pick_files_filter_spec() {
        assert_eq!(
            join_specification(&[
                "zip", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "7z"
            ]),
            "zip,tar,tar.gz,tgz,tar.bz2,tbz2,tar.xz,txz,7z"
        );
        assert_eq!(join_specification(&["", "zip", "", "tar"]), "zip,tar");
        assert_eq!(join_specification(&[]), "");
    }
}
