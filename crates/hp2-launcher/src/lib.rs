//! Rust launcher policy, model, store, complete pre-launch round, import-session
//! control, desktop presentation/event policy, and application main-loop policy.
//! Native code retains typed SDL/ImGui leaves, SDK object storage, OS dialogs,
//! lexical FTime and guard scopes, and joined worker/resource lifetimes.

pub mod atomic;
pub mod boot;
pub mod boot_config;
pub mod capabilities;
pub mod command_policy;
pub mod display_modes;
pub mod feedback;
pub mod import_session;
pub mod launch_round;
pub mod main_loop;
pub mod migrate;
pub mod model;
pub mod orchestrate;
pub mod policy;
pub mod presentation_policy;
pub mod profile;
pub mod reporters;
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
