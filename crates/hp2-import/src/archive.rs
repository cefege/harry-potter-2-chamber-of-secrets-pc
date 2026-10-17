//! Replaces `HP2ImportArchive.cpp`: `MemberPath` (pure path-safety
//! validation, always compiled) and `Extract` (the real libarchive-
//! backed extraction, behind the `archive` Cargo feature -- see
//! `Cargo.toml`'s `[features]` doc comment for why).
//!
//! **Design approved by the user (2026-10-17), recorded here and in
//! `crate::lib`'s "Approved deviations" doc section: hand-written
//! `extern "C"` bindings against the pinned `archive_static` (built by
//! CMake's FetchContent at the libarchive commit `HP2Dependencies.cmake`
//! pins), not the plan-named `compress-tools` crate.** `compress-tools`
//! was confirmed to have no vendoring path at all: its `build.rs` hard-
//! requires system `pkg-config` libarchive, and no `libarchive.pc`
//! exists anywhere in the pinned FetchContent build tree either -- it
//! cannot consume this project's already-pinned, already-built
//! `archive_static` at all.
//!
//! The `ffi` module's declarations were written directly against the
//! pinned libarchive source's headers (`archive.h`/`archive_entry.h` at
//! the exact pinned commit, not a generic/latest-version reference) --
//! function names, parameter types, and the two struct layouts
//! (`archive`/`archive_entry` are always used as `*mut`, treated as
//! opaque by every libarchive caller including the real C++, so a
//! zero-sized opaque struct is the correct, exact representation, not
//! an approximation of one). `__LA_MODE_T` is `mode_t`, which differs
//! in width between macOS (`u16`) and Linux (`u32`) -- matched per-
//! platform via `cfg`, not assumed to be one fixed width, since this
//! project supports both (see `AGENTS.md`'s platform table).
//!
//! Verified against the real compiled `Extract`/`MemberPath`: a
//! throwaway probe crate linked directly against the already-built
//! `_deps/hp2_libarchive-build/libarchive/libarchive.a` (same pinned
//! library CMake's own build already produces, not a separately
//! fetched copy, via a throwaway `build.rs` supplying the link-search/
//! link-lib directives -- not part of this crate; see the commit
//! message for why a committed one would be premature here), run
//! against real ZIP and TAR fixtures covering twelve cases and
//! confirming byte-identical results on every one: the happy path
//! (ZIP and TAR separately), an unsupported extension, a TAR archived
//! with a literal `.` self-entry (`tar -C dir .`, a real-world
//! footgun, not a contrived case), a tree missing every required
//! category directory (zero candidates), a tree with two complete
//! candidate trees at different nesting depths, a case-insensitive
//! duplicate member (`System/File.txt` and `system/file.txt`), a
//! path-traversal member (`../evil.txt`), a symlink entry (TAR, real
//! `ln -s` dereferenced by `tar`), an absolute-path member
//! (`/abs/evil.txt`), a TAR hardlink entry (`tarfile.LNKTYPE`), and a
//! ZIP member whose name ends in `/` despite being marked as a regular
//! file via `external_attr` -- libarchive itself (not this port's own
//! logic) classifies that last one as a directory regardless of the
//! attribute override, and both implementations agree exactly on the
//! resulting (empty) directory tree, confirming this port reads
//! whatever libarchive reports rather than second-guessing it.

use crate::util::fail;

/// Replaces `MemberPath`: rejects an archive member name that is empty,
/// absolute, drive-qualified, contains a `.`/`..`/empty path component,
/// or a component with an embedded NUL byte -- every way a malicious or
/// malformed archive entry could otherwise escape `destination` or
/// confuse the filesystem. No FFI dependency (pure string logic, unlike
/// the rest of this module), so always compiled and tested regardless
/// of the `archive` feature.
pub fn member_path(name: &str) -> Result<String, String> {
    let name = name.replace('\\', "/");
    if name.is_empty() || name.starts_with('/') {
        return Err(fail(format!("unsafe archive member path: '{name}'")));
    }
    // `std::getline(stream, part, '/')` stops without producing a final
    // part when the stream reaches EOF exactly at a delimiter with
    // nothing left to read afterward -- confirmed against the real
    // compiled `MemberPath`: a single trailing '/' is silently dropped
    // ("a/b/" -> "a/b", not an error), but a *second* trailing '/'
    // still reports the empty component between them as an error
    // ("a/b//" -> error, "a//" -> error). `str::split` always reports
    // that final empty element; dropping exactly one trailing empty
    // element when `name` itself ends with '/' reproduces getline's
    // behavior exactly, including the two-slash case (which still has
    // a now-un-dropped empty element in the middle of the result).
    let mut parts: Vec<&str> = name.split('/').collect();
    if name.ends_with('/') {
        parts.pop();
    }
    let mut normalized = String::new();
    let mut first = true;
    for part in parts {
        if part.is_empty() || part == "." || part == ".." {
            return Err(fail(format!("unsafe archive member path: '{name}'")));
        }
        if first && part.contains(':') {
            return Err(fail(format!(
                "drive-qualified archive member path: '{name}'"
            )));
        }
        if part.contains('\0') {
            return Err(fail(format!("unsafe path component: '{part}'")));
        }
        if !first {
            normalized.push('/');
        }
        normalized.push_str(part);
        first = false;
    }
    Ok(normalized)
}

#[cfg(feature = "archive")]
#[allow(non_camel_case_types, non_upper_case_globals, dead_code)]
mod ffi {
    use std::os::raw::{c_char, c_int, c_void};

    #[cfg(target_os = "macos")]
    pub type LaModeT = u16;
    #[cfg(target_os = "linux")]
    pub type LaModeT = u32;

    pub const ARCHIVE_OK: c_int = 0;
    pub const ARCHIVE_EOF: c_int = 1;
    pub const AE_IFREG: LaModeT = 0o100000;
    pub const AE_IFDIR: LaModeT = 0o040000;

    /// Opaque; every libarchive caller (including the real C++) only
    /// ever holds a pointer to this, never reads its layout.
    #[repr(C)]
    pub struct Archive {
        _private: [u8; 0],
    }
    #[repr(C)]
    pub struct ArchiveEntry {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn archive_read_new() -> *mut Archive;
        pub fn archive_read_support_format_zip(a: *mut Archive) -> c_int;
        pub fn archive_read_support_format_tar(a: *mut Archive) -> c_int;
        pub fn archive_read_support_format_7zip(a: *mut Archive) -> c_int;
        pub fn archive_read_support_filter_none(a: *mut Archive) -> c_int;
        pub fn archive_read_support_filter_gzip(a: *mut Archive) -> c_int;
        pub fn archive_read_support_filter_bzip2(a: *mut Archive) -> c_int;
        pub fn archive_read_support_filter_xz(a: *mut Archive) -> c_int;
        pub fn archive_read_open_filename(
            a: *mut Archive,
            filename: *const c_char,
            block_size: usize,
        ) -> c_int;
        pub fn archive_read_next_header(a: *mut Archive, entry: *mut *mut ArchiveEntry) -> c_int;
        pub fn archive_read_data_skip(a: *mut Archive) -> c_int;
        pub fn archive_read_data(a: *mut Archive, buff: *mut c_void, size: usize) -> isize;
        pub fn archive_read_free(a: *mut Archive) -> c_int;
        pub fn archive_error_string(a: *mut Archive) -> *const c_char;
        pub fn archive_entry_pathname(entry: *mut ArchiveEntry) -> *const c_char;
        pub fn archive_entry_pathname_utf8(entry: *mut ArchiveEntry) -> *const c_char;
        pub fn archive_entry_filetype(entry: *mut ArchiveEntry) -> LaModeT;
        pub fn archive_entry_hardlink(entry: *mut ArchiveEntry) -> *const c_char;
        pub fn archive_entry_symlink(entry: *mut ArchiveEntry) -> *const c_char;
    }
}

#[cfg(feature = "archive")]
mod extract {
    use super::ffi::*;
    use super::member_path;
    use crate::lower::lower;
    use crate::util::{fail, strerror_of, utf8};
    use std::collections::BTreeSet;
    use std::ffi::{CStr, CString};
    use std::fs;
    use std::io::Write;
    use std::os::raw::c_char;
    use std::path::{Path, PathBuf};

    const CHUNK_SIZE: usize = 1024 * 1024;

    /// SAFETY wrapper: frees the reader on drop, matching every one of
    /// the C++ function's many early-return `archive_read_free(reader);
    /// Fail(...)` pairs with a single RAII point instead of repeating
    /// the free call at each of them.
    struct Reader(*mut Archive);
    impl Drop for Reader {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    archive_read_free(self.0);
                }
            }
        }
    }

    /// Replaces `ArchiveError`.
    fn archive_error(reader: *mut Archive) -> String {
        unsafe {
            let ptr = archive_error_string(reader);
            if ptr.is_null() {
                "unknown archive error".to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    /// Replaces `EntryName`.
    fn entry_name(entry: *mut ArchiveEntry) -> String {
        unsafe {
            let utf8_ptr = archive_entry_pathname_utf8(entry);
            let ptr = if !utf8_ptr.is_null() {
                utf8_ptr
            } else {
                archive_entry_pathname(entry)
            };
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    unsafe fn c_str_present(ptr: *const c_char) -> bool {
        !ptr.is_null()
    }

    /// Replaces `Extract`.
    pub fn extract(file: &Path, destination: &Path) -> Result<PathBuf, String> {
        let filename_str = utf8(file)?;
        let extension = lower(&filename_str);
        let supported = [
            ".7z", ".zip", ".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz",
        ];
        if !supported.iter().any(|ext| extension.ends_with(ext)) {
            return Err(fail(format!(
                "unsupported archive format (use ZIP, TAR, or 7z): {filename_str}"
            )));
        }

        let raw = unsafe { archive_read_new() };
        if raw.is_null() {
            return Err(fail(format!(
                "cannot read archive {filename_str}: cannot allocate archive reader"
            )));
        }
        let reader = Reader(raw);
        unsafe {
            archive_read_support_format_zip(raw);
            archive_read_support_format_tar(raw);
            archive_read_support_format_7zip(raw);
            archive_read_support_filter_none(raw);
            archive_read_support_filter_gzip(raw);
            archive_read_support_filter_bzip2(raw);
            archive_read_support_filter_xz(raw);
        }
        let c_filename = CString::new(filename_str.clone())
            .map_err(|_| fail(format!("cannot read archive {filename_str}: embedded NUL")))?;
        let open_result =
            unsafe { archive_read_open_filename(raw, c_filename.as_ptr(), CHUNK_SIZE) };
        if open_result != ARCHIVE_OK {
            let reason = archive_error(raw);
            return Err(fail(format!(
                "cannot read archive {filename_str}: {reason}"
            )));
        }

        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut buffer = vec![0u8; CHUNK_SIZE];
        loop {
            let mut entry: *mut ArchiveEntry = std::ptr::null_mut();
            let result = unsafe { archive_read_next_header(raw, &mut entry) };
            if result == ARCHIVE_EOF {
                break;
            }
            if result != ARCHIVE_OK {
                let reason = archive_error(raw);
                return Err(fail(format!(
                    "cannot read archive {filename_str}: {reason}"
                )));
            }
            let mut raw_name = entry_name(entry);
            let directory = unsafe { archive_entry_filetype(entry) } == AE_IFDIR;
            if directory {
                while raw_name.ends_with('/') || raw_name.ends_with('\\') {
                    raw_name.pop();
                }
            }
            let relative = member_path(&raw_name)?;
            let key = lower(&relative);
            if !seen.insert(key) {
                return Err(fail(format!(
                    "case-insensitive duplicate archive member: {raw_name}"
                )));
            }
            let filetype = unsafe { archive_entry_filetype(entry) };
            let has_hardlink = unsafe { c_str_present(archive_entry_hardlink(entry)) };
            let has_symlink = unsafe { c_str_present(archive_entry_symlink(entry)) };
            if has_hardlink || has_symlink || (filetype != AE_IFDIR && filetype != AE_IFREG) {
                return Err(fail(format!(
                    "archive link/device entry is forbidden: {raw_name}"
                )));
            }
            let target = destination.join(&relative);
            if directory {
                fs::create_dir_all(&target).map_err(|e| {
                    fail(format!(
                        "cannot extract archive member {raw_name}: {}",
                        strerror_of(&e)
                    ))
                })?;
                unsafe {
                    archive_read_data_skip(raw);
                }
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    fail(format!(
                        "cannot extract archive member {raw_name}: {}",
                        strerror_of(&e)
                    ))
                })?;
            }
            if target.exists() {
                return Err(fail(format!(
                    "cannot extract archive member {raw_name}: exclusive create failed"
                )));
            }
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|_| {
                    fail(format!(
                        "cannot extract archive member {raw_name}: exclusive create failed"
                    ))
                })?;
            loop {
                let amount =
                    unsafe { archive_read_data(raw, buffer.as_mut_ptr().cast(), buffer.len()) };
                if amount == 0 {
                    break;
                }
                if amount < 0 {
                    let reason = archive_error(raw);
                    return Err(fail(format!("cannot read archive member: {reason}")));
                }
                output.write_all(&buffer[..amount as usize]).map_err(|_| {
                    fail(format!(
                        "cannot extract archive member {raw_name}: write failed"
                    ))
                })?;
            }
            output.flush().map_err(|_| {
                fail(format!(
                    "cannot extract archive member {raw_name}: write failed"
                ))
            })?;
        }
        drop(reader);

        let required = ["system", "maps", "textures", "sounds", "music", "help"];
        let mut candidates: Vec<PathBuf> = Vec::new();
        let mut directories: Vec<PathBuf> = vec![destination.to_path_buf()];
        while let Some(current) = directories.pop() {
            let mut children: BTreeSet<String> = BTreeSet::new();
            let entries = fs::read_dir(&current).map_err(|e| {
                fail(format!(
                    "cannot inspect extracted archive: {}: {}",
                    utf8(&current).unwrap_or_else(|_| current.display().to_string()),
                    strerror_of(&e)
                ))
            })?;
            for entry in entries {
                let entry = entry.map_err(|e| {
                    fail(format!(
                        "cannot inspect extracted archive: {}: {}",
                        utf8(&current).unwrap_or_else(|_| current.display().to_string()),
                        strerror_of(&e)
                    ))
                })?;
                let path = entry.path();
                let meta = entry.metadata().map_err(|e| {
                    fail(format!(
                        "cannot inspect extracted archive: {}: {}",
                        utf8(&path).unwrap_or_else(|_| path.display().to_string()),
                        strerror_of(&e)
                    ))
                })?;
                if meta.file_type().is_symlink() {
                    return Err(fail(format!(
                        "symbolic-link directory in extracted archive: {}",
                        utf8(&path).unwrap_or_else(|_| path.display().to_string())
                    )));
                }
                if meta.is_dir() {
                    let name = lower(
                        &utf8(&path.file_name().map(PathBuf::from).unwrap_or_default())
                            .unwrap_or_default(),
                    );
                    if !children.insert(name) {
                        return Err(fail(format!(
                            "case-insensitive duplicate archive member: {}",
                            utf8(&path.file_name().map(PathBuf::from).unwrap_or_default())
                                .unwrap_or_default()
                        )));
                    }
                    directories.push(path);
                }
            }
            if required.iter().all(|r| children.contains(*r)) {
                candidates.push(current);
            }
        }
        if candidates.len() != 1 {
            return Err(fail(format!(
                "expected one retail tree in archive, found {}",
                candidates.len()
            )));
        }
        Ok(candidates.into_iter().next().expect("len checked above"))
    }
}

#[cfg(feature = "archive")]
pub use extract::extract;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_path_normalizes_backslashes_to_forward_slashes() {
        assert_eq!(member_path("a\\b\\c").unwrap(), "a/b/c");
    }

    #[test]
    fn member_path_rejects_empty_and_absolute_paths() {
        assert!(member_path("").is_err());
        assert!(member_path("/etc/passwd").is_err());
    }

    #[test]
    fn member_path_rejects_dot_and_dotdot_components() {
        assert!(member_path("a/./b").is_err());
        assert!(member_path("a/../b").is_err());
        assert!(member_path("../etc/passwd").is_err());
    }

    #[test]
    fn member_path_rejects_an_empty_component_from_a_doubled_internal_separator() {
        assert!(member_path("a//b").is_err());
    }

    /// Confirmed against the real compiled `MemberPath`: a single
    /// trailing `/` does not leave a trailing empty path component the
    /// way a naive `str::split('/')` would -- `std::getline` simply
    /// stops, having nothing left to read, rather than reporting one
    /// more (empty) part.
    #[test]
    fn member_path_silently_drops_a_single_trailing_separator() {
        assert_eq!(member_path("a/b/").unwrap(), "a/b");
        assert_eq!(member_path("a/").unwrap(), "a");
        assert_eq!(member_path("a/b/c/").unwrap(), "a/b/c");
    }

    /// Confirmed against the real compiled `MemberPath`: unlike a
    /// single trailing `/`, a *second* consecutive trailing `/` is
    /// still an empty-component error -- the first of the two is
    /// absorbed by `getline` finding more stream to read (an empty
    /// part, successfully extracted), and only the truly-final,
    /// nothing-left-to-read position is silently dropped.
    #[test]
    fn member_path_rejects_a_doubled_trailing_separator() {
        assert!(member_path("a/b//").is_err());
        assert!(member_path("a//").is_err());
    }

    #[test]
    fn member_path_rejects_a_drive_qualified_first_component() {
        let err = member_path("C:/Windows/System32").unwrap_err();
        assert!(
            err.starts_with("drive-qualified archive member path:"),
            "{err}"
        );
    }

    #[test]
    fn member_path_allows_a_colon_in_a_non_first_component() {
        // Only the first component is checked for a drive letter --
        // matches the real C++'s `if (first && part.find(':') != ...)`.
        assert_eq!(member_path("a/b:c").unwrap(), "a/b:c");
    }

    #[test]
    fn member_path_rejects_an_embedded_nul_byte() {
        let err = member_path("a/b\0c/d").unwrap_err();
        assert!(err.starts_with("unsafe path component:"), "{err}");
    }

    #[test]
    fn member_path_accepts_an_ordinary_nested_path() {
        assert_eq!(member_path("System/Core.u").unwrap(), "System/Core.u");
    }
}
