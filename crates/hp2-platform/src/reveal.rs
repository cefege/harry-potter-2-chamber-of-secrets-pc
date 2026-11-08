//! Reveal a path in the desktop file manager, replacing
//! `HP2LauncherPlatform::RevealPath`'s non-Apple body
//! (`SDLLaunch/Src/HP2LauncherPlatform.cpp:85-99`, deleted) and the Apple
//! body that used to live in `HP2LauncherPlatformMac.mm` (deleted), a
//! five-line Cocoa `NSWorkspace openURL:` call. The macOS branch below binds
//! `libobjc`/Foundation/AppKit directly (`objc_getClass`/`sel_registerName`/
//! `objc_msgSend`, cast to each call's exact signature) rather than linking
//! an Objective-C++ translation unit; `objc_msgSend` itself is safe to call
//! this way on arm64, which has no separate `objc_msgSend_stret` ABI for
//! non-struct-returning methods (every method used here returns a pointer or
//! `BOOL`). The real C++'s `_WIN32` `ShellExecuteW` branch has no
//! replacement here: this port's finish line is macOS/Linux only
//! (`local://round8-contract.md`), so a target that is neither macOS nor
//! Linux falls into the real C++'s own catch-all `#else (void)Path;`
//! branch, i.e. a no-op, same as the original on any such platform.

#[cfg(all(target_os = "linux", feature = "sdl"))]
unsafe extern "C" {
    // SDL2 `SDL_log.h`; same hand-written binding `picker.rs` declares,
    // behind the same `sdl` feature. Variadic, called with one
    // NUL-terminated `*const c_char` for `%s` and one `i32` for `%d`,
    // matching the original's own `SDL_Log(fmt, Path.c_str(), status)`.
    fn SDL_Log(fmt: *const std::os::raw::c_char, ...);
}

/// `RevealPath`'s `if (Path.empty()) return;` early return, shared by the
/// Linux spawn path (`HP2LauncherPlatform.cpp:87`) and the macOS Cocoa path
/// (`HP2LauncherPlatformMac.mm:9`, same check verbatim). A pure predicate so
/// the no-op case is directly testable without spawning a process or
/// touching the Objective-C runtime.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn should_reveal(path: &[u8]) -> bool {
    !path.is_empty()
}

/// `RevealPath`'s `#elif defined(__linux__)` body
/// (`HP2LauncherPlatform.cpp:90-94`). `path` is the real C++'s raw
/// `std::string` bytes, not validated UTF-8 (`posix_spawnp` takes the raw
/// bytes via `Path.c_str()` directly). Spawns `xdg-open` and does not wait
/// for it, matching the original fire-and-forget `posix_spawnp` (the
/// child's `pid_t` is captured by the real C++ only to satisfy
/// `posix_spawnp`'s out-parameter; it is never reaped).
#[cfg(target_os = "linux")]
pub fn reveal_path(path: &[u8]) {
    if !should_reveal(path) {
        return;
    }
    // `Path.c_str()` is NUL-terminated by construction; `path` here is a
    // borrowed byte span, so build an equivalent NUL-terminated C string,
    // truncated at the first embedded NUL exactly as a C string read of
    // `Path.c_str()` would stop there too.
    let nul_at = path.iter().position(|&b| b == 0).unwrap_or(path.len());
    // SAFETY: `path[..nul_at]` has no interior NUL by construction, so
    // `CString::new` cannot fail.
    let argument = std::ffi::CString::new(&path[..nul_at]).unwrap();
    let program = c"xdg-open";
    // `char* const arguments[] = {const_cast<char*>("xdg-open"),
    // const_cast<char*>(Path.c_str()), nullptr};` (`HP2LauncherPlatform.cpp:91`).
    let argv: [*mut std::os::raw::c_char; 3] = [
        program.as_ptr() as *mut std::os::raw::c_char,
        argument.as_ptr() as *mut std::os::raw::c_char,
        std::ptr::null_mut(),
    ];
    let mut child: libc::pid_t = 0;
    // `posix_spawnp(&child, "xdg-open", nullptr, nullptr, arguments,
    // environ)` (`HP2LauncherPlatform.cpp:92`), called directly rather than
    // through `std::process::Command`: `Command`'s own spawn path (which may
    // fall back to fork+exec, and wraps failures in `io::Error`) does not
    // retrace the original's exact `posix_spawnp` return value, which the
    // original logs verbatim as `%d`.
    //
    // SAFETY: `program` and `argument` are valid NUL-terminated C strings
    // alive for the duration of this call; `argv`'s last element is a NUL
    // sentinel; `environ` is libc's process-wide environment pointer.
    let status = unsafe {
        libc::posix_spawnp(
            &mut child,
            program.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            argv.as_ptr(),
            libc::environ,
        )
    };
    // `if (status != 0) SDL_Log("HP2Launcher: could not start xdg-open for
    // %s (error %d)", Path.c_str(), status);` (`HP2LauncherPlatform.cpp:94`).
    // `status` here is `posix_spawnp`'s own return value (0 or an errno
    // number), not an `errno` read after the fact.
    if status != 0 {
        #[cfg(feature = "sdl")]
        // SAFETY: `fmt` is a NUL-terminated string literal matching the
        // original's format exactly; `argument` is a valid NUL-terminated
        // C string for the duration of this call.
        unsafe {
            SDL_Log(
                c"HP2Launcher: could not start xdg-open for %s (error %d)".as_ptr(),
                argument.as_ptr(),
                status,
            );
        }
    }
}
/// `RevealPath`'s Apple body (`HP2LauncherPlatformMac.mm:7-18`, deleted).
/// `path` is the real C++'s raw `std::string` bytes, not validated UTF-8
/// (`[NSString stringWithUTF8String:]` takes `Path.c_str()` directly and
/// returns `nil` on invalid UTF-8, matching the early return below on a
/// null `NSString`). Opens the path in Finder via `NSWorkspace openURL:`;
/// on failure, logs through `NSLog` with the same format string as the
/// original. Builds and tears down its own autorelease pool, matching the
/// original's `@autoreleasepool { ... }` block.
#[cfg(target_os = "macos")]
pub fn reveal_path(path: &[u8]) {
    use std::ffi::c_char;
    use std::os::raw::c_void;

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc", kind = "dylib")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    #[link(name = "Foundation", kind = "framework")]
    unsafe extern "C" {
        // `NSLog(NSString*, ...)`; the real C++ passes one `%@` argument,
        // another `NSString*`.
        fn NSLog(format: Id, ...);
    }

    // `AppKit` (for `NSWorkspace`) re-exports `Foundation`'s classes
    // (`NSString`, `NSURL`); linking it is what registers `NSWorkspace`
    // with the Objective-C runtime so `objc_getClass("NSWorkspace")` below
    // can find it.
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    // SAFETY: each closure below is called immediately with `objc_msgSend`
    // reinterpreted as the exact signature Objective-C would dispatch for
    // that selector; arm64 has one `objc_msgSend` entry point for every
    // non-struct-returning message (no `_stret` variant to pick between).
    unsafe fn send0(receiver: Id, sel: Sel) -> Id {
        let f: unsafe extern "C" fn(Id, Sel) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(receiver, sel) }
    }
    unsafe fn send_cstr(receiver: Id, sel: Sel, arg: *const c_char) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(receiver, sel, arg) }
    }
    unsafe fn send_id(receiver: Id, sel: Sel, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(receiver, sel, arg) }
    }
    unsafe fn send_id_bool(receiver: Id, sel: Sel, arg: Id) -> bool {
        let f: unsafe extern "C" fn(Id, Sel, Id) -> u8 =
            unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(receiver, sel, arg) != 0 }
    }

    // `if (Path.empty()) return;` (`HP2LauncherPlatformMac.mm:9`).
    if !should_reveal(path) {
        return;
    }
    // `NSString stringWithUTF8String:` wants a NUL-terminated UTF-8 buffer,
    // matching `Path.c_str()`; truncate at the first embedded NUL exactly
    // as a C string read of `Path.c_str()` would.
    let nul_at = path.iter().position(|&b| b == 0).unwrap_or(path.len());
    let Ok(path_cstring) = std::ffi::CString::new(&path[..nul_at]) else {
        return;
    };

    // SAFETY: every selector/class name below is a NUL-terminated string
    // literal; every receiver is either a looked-up class or a value
    // returned by a prior Objective-C message send on this same thread,
    // scoped to the autorelease pool pushed immediately below and popped
    // before returning.
    unsafe {
        let pool = objc_autoreleasePoolPush();

        let ns_string_class = objc_getClass(c"NSString".as_ptr());
        let sel_string_with_utf8 = sel_registerName(c"stringWithUTF8String:".as_ptr());
        // `NSString* utf8Path = [NSString stringWithUTF8String:Path.c_str()];`
        let utf8_path = send_cstr(ns_string_class, sel_string_with_utf8, path_cstring.as_ptr());
        // `if (!utf8Path) return;`
        if utf8_path.is_null() {
            objc_autoreleasePoolPop(pool);
            return;
        }

        let ns_url_class = objc_getClass(c"NSURL".as_ptr());
        let sel_file_url_with_path = sel_registerName(c"fileURLWithPath:".as_ptr());
        // `NSURL* url = [NSURL fileURLWithPath:utf8Path];`
        let url = send_id(ns_url_class, sel_file_url_with_path, utf8_path);

        let ns_workspace_class = objc_getClass(c"NSWorkspace".as_ptr());
        let sel_shared_workspace = sel_registerName(c"sharedWorkspace".as_ptr());
        // `[NSWorkspace sharedWorkspace]`
        let workspace = send0(ns_workspace_class, sel_shared_workspace);

        let sel_open_url = sel_registerName(c"openURL:".as_ptr());
        // `if (![[NSWorkspace sharedWorkspace] openURL:url])`
        let opened = send_id_bool(workspace, sel_open_url, url);
        if !opened {
            // `NSLog(@"HP2Launcher: could not reveal path %@", utf8Path);`
            let sel_string_with_utf8_format =
                sel_registerName(c"stringWithUTF8String:".as_ptr());
            let format = send_cstr(
                ns_string_class,
                sel_string_with_utf8_format,
                c"HP2Launcher: could not reveal path %@".as_ptr(),
            );
            NSLog(format, utf8_path);
        }

        objc_autoreleasePoolPop(pool);
    }
}

/// No-op on every target reachable from this crate that is neither Linux
/// nor macOS (this port's finish line is macOS/Linux only).
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn reveal_path(_path: &[u8]) {}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use super::*;

    /// Empty path is an early return (`HP2LauncherPlatform.cpp:87`): no
    /// process spawn attempt. Asserts the exact predicate `reveal_path`
    /// branches on, not just that the call doesn't panic.
    #[test]
    fn empty_path_is_a_no_op() {
        assert!(!should_reveal(b""));
    }

    /// A non-empty path is not the early-return case.
    #[test]
    fn non_empty_path_would_spawn() {
        assert!(should_reveal(b"/tmp/example"));
    }
}
