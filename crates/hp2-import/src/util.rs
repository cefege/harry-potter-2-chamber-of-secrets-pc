//! Small helpers shared across this crate's modules: path/UTF-8
//! conversions, failure-message construction, and (behind `#[cfg(test)]`)
//! a scratch-directory helper every module's tests use.

use std::path::{Path, PathBuf};

use crate::lower::lower;

/// Replaces `Fail(s)`/`throw Failure{s}` as a plain `Err(String)`
/// instead of an exception: every function in this crate returns
/// `Result<_, String>` rather than throwing, so this just gives the
/// construction a name matching the C++ it replaces.
pub(crate) fn fail(message: impl Into<String>) -> String {
    message.into()
}

/// Matches `Utf8(path)`. `None` (mapped to an `Err` by the caller) stands
/// in for the C++ side's implementation-defined handling of a native path
/// that is not valid UTF-8 -- see `crate::files`'s module doc.
pub(crate) fn utf8(p: &Path) -> Result<String, String> {
    p.to_str()
        .map(str::to_string)
        .ok_or_else(|| fail(format!("non-UTF-8 path: {}", p.to_string_lossy())))
}

/// `std::error_code::message()`/`ec.message()` equivalent for a Rust
/// `io::Error`: the raw OS text via the shared, thread-safe,
/// Windows-correct `hp2_ffi_support::strerror`, *not* `io::Error`'s own
/// `Display`, which appends `" (os error N)"` that C++ never produces --
/// any stderr-text comparison (e.g. `importer_parity`) would see that
/// difference. Falls back to `io::Error`'s own text only when there's no
/// OS error code at all (a Rust-internal error with no `errno`/Win32
/// code behind it, which `std::error_code` has no equivalent of either).
pub(crate) fn strerror_of(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(code) => hp2_ffi_support::strerror(code),
        None => e.to_string(),
    }
}

/// Matches `fs::path::extension()`'s string (including the leading `.`,
/// or empty when there is none), lowercased via [`crate::lower::lower`].
/// Rust's `Path::extension()` omits the dot and uses the same "no dot"
/// special case for a name with no extension or a dotfile like
/// `.gitignore`; verified against the real `fs::path::extension()` on
/// five cases including a trailing-dot name (`foo.` -> `"."` on both
/// sides) and a multi-dot name (only the final `.ext` counts).
///
/// Every caller in this crate already passes a path whose filename came
/// from a `walk`/`deterministic_files` result (already `utf8()?`-
/// checked), so the non-UTF-8 fallback (empty string) below is a
/// defensive default for a hypothetical future caller, not a case this
/// crate's own call sites can hit; the assert makes a caller that does
/// break that precondition fail loudly in tests.
pub(crate) fn lower_extension(path: &Path) -> String {
    debug_assert!(
        path.extension().is_none_or(|e| e.to_str().is_some()),
        "lower_extension's non-UTF-8 fallback is being hit: {}",
        path.display()
    );
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => lower(&format!(".{ext}")),
        None => String::new(),
    }
}

/// Replaces `fs::path::lexically_relative`, purely by comparing path
/// components -- no filesystem access, matching the C++ function. Rust's
/// `Path::strip_prefix` is not a substitute: it returns `Err` (not an
/// empty path) for a path outside `base`, and never produces a `../`
/// prefix, so naively falling back to the original path on its `Err`
/// would silently accept an escaping path with the absolute path left in
/// a destination string instead of erroring. Used everywhere this crate
/// computes a path relative to a known base -- not just
/// `collect::make_asset`'s security-relevant root-escape check, but
/// `files::sort_by_relative_path` and `collect::collect_to`'s ordinary
/// relative-path computation too, so there is exactly one fallback
/// behavior for "path turned out not to be under base" across the
/// crate, not a different one in each call site.
///
/// `None` matches C++ returning an empty path: `base`/`path` disagree on
/// absolute-vs-relative (the only mismatch this project's inputs can hit
/// in practice; both come from directory walks under an absolute root).
/// Otherwise returns `base`'s number of non-shared trailing components
/// worth of `..`, followed by `path`'s own non-shared trailing
/// components -- e.g. `/a/b/c` relative to `/a/x/y` is `../../b/c`. When
/// `path` is genuinely under `base` (the common case: a file `walk`/
/// `deterministic_files` found while listing `base` itself), this is
/// `base`'s whole component list as the shared prefix and zero `..`
/// components -- the exact same result `strip_prefix` would give, so
/// using this everywhere instead of `strip_prefix` changes nothing for
/// well-formed inputs and only changes behavior (from a silent wrong
/// answer to a visible one) for a path that turns out not to be under
/// base after all.
pub(crate) fn lexically_relative(base: &Path, path: &Path) -> Option<PathBuf> {
    if base.is_absolute() != path.is_absolute() {
        return None;
    }
    let base_components: Vec<_> = base.components().collect();
    let path_components: Vec<_> = path.components().collect();
    let common = base_components
        .iter()
        .zip(path_components.iter())
        .take_while(|(a, b)| a == b)
        .count();
    let mut result = PathBuf::new();
    for _ in 0..(base_components.len() - common) {
        result.push("..");
    }
    for component in &path_components[common..] {
        result.push(component.as_os_str());
    }
    Some(result)
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Minimal `tempfile`-free scratch directory: a unique path under the
    /// OS temp dir, removed on drop. Unique per call via `process::id()`
    /// plus a monotonic counter, not wall-clock time: tests in one test
    /// binary run in parallel by default, and two calls close enough in
    /// time could otherwise collide on the same directory name, making
    /// one test's `Drop` (`remove_dir_all`) race another's still-running
    /// body -- an intermittent failure, which AGENTS.md treats as a
    /// finding, not noise to retry past.
    pub(crate) struct TempDir(PathBuf);

    impl TempDir {
        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    pub(crate) fn tempdir() -> TempDir {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "hp2-import-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    pub(crate) fn write(root: &Path, rel: &str, content: &[u8]) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(path).unwrap().write_all(content).unwrap();
    }

    #[test]
    fn tempdir_names_never_collide_across_many_rapid_calls() {
        let mut seen = std::collections::HashSet::new();
        let mut dirs = Vec::new();
        for _ in 0..200 {
            let dir = tempdir();
            assert!(
                seen.insert(dir.path().to_path_buf()),
                "duplicate tempdir name: {}",
                dir.path().display()
            );
            dirs.push(dir);
        }
    }
}
