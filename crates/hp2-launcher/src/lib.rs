//! Rust port of `HarryPotter2/Unreal/SDLLaunch/Src/HP2Launch*.cpp`'s
//! pure logic (policy, store, model) -- excludes `HP2Launcher.cpp`'s
//! own SDL/ImGui UI loop, which stays C++ per the plan.
//!
//! **Known dead code in the real C++, not ported:**
//! `HP2LaunchPolicy.h` declares `ParseLauncherAutomationRequest` and
//! `BuildLauncherAutomationResult` (and the `LauncherAutomationRequest`/
//! `LauncherAutomationParseResult` types supporting them), but neither
//! function has a definition anywhere in this repository, and neither
//! the functions nor the types they use are referenced from any other
//! `.cpp`/`.mm` file (confirmed via `grep -rln` across the whole repo).
//! This is vestigial -- declared, never implemented, never called --
//! not a gap in this port.

pub mod atomic;
pub mod migrate;
pub mod model;
pub mod policy;
pub mod profile;
pub mod saves;
pub mod store;
pub mod tree;

/// Minimal `tempfile`-free scratch directory shared by `atomic`'s and
/// `tree`'s tests (both need real filesystem operations): a unique
/// path under the OS temp dir, removed on drop. Unique per call via
/// `process::id()` plus a monotonic counter, not wall-clock time --
/// tests in one test binary run in parallel by default, and two calls
/// close enough in time could otherwise collide on the same directory
/// name. Same technique `hp2_import::util::test_support` already
/// established in that crate (duplicated here rather than shared
/// across crates since that helper is `pub(crate)` to its own crate).
#[cfg(test)]
pub(crate) mod test_support {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

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
            "hp2-launcher-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}
