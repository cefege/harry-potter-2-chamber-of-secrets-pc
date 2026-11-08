//! Rust port of `HarryPotter2/Unreal/Import/Src/HP2Import*.cpp` (the
//! `prepare_retail_data.py`-equivalent importer that builds the runtime's
//! `HarryPotter2/Unreal` overlay). Ported incrementally, one piece at a
//! time; see the plan for the full scope.
//!
//! Three pieces of this crate deviate from the plan's literal text or
//! from a byte-for-byte C++ transcription. The plan document itself is
//! a session-local artifact, not part of this repo, so the decisions
//! are recorded here instead of just pointed at:
//!
//! - **Archive extraction's crate choice** (implemented; see
//!   [`archive`]). Plan text: the
//!   `compress-tools` crate, pinned in Cargo instead of FetchContent.
//!   Tested empirically: `compress-tools` (0.15.1 and 0.16.1, source
//!   read directly) has no vendoring or `cc`-based build at all -- on
//!   every non-MSVC target it hard-requires a system libarchive
//!   discoverable via `pkg-config`, not installed on this dev machine
//!   and not part of this project's dependency story anywhere else
//!   (every other native dependency is FetchContent-pinned to an exact
//!   commit, built in-tree). Presented to the user in plain language;
//!   **user-approved 2026-10-17**: hand-written `extern "C"` bindings
//!   against the existing pinned `archive_static`
//!   (`Build/CMake/HP2Dependencies.cmake`'s `hp2_libarchive`), behind a
//!   Cargo feature gate so `cargo test -p hp2-import` keeps working
//!   with no libarchive present. libarchive stays in
//!   `HP2Dependencies.cmake`, unchanged.
//! - **The casefold table's data source** (implemented; see [`lower`]
//!   and `fold_table`). Plan text: the `unicode-normalization` crate.
//!   Tried `caseless` (built on `unicode-normalization`) first, per the
//!   plan; an exhaustive diff against the pinned utf8proc 2.12.0's
//!   actual `UTF8PROC_CASEFOLD` output for every Unicode scalar value
//!   found 146 real divergences (Unicode additions newer than
//!   `unicode-normalization`'s bundled tables, e.g. the Garay script).
//!   Presented to the user in plain language; **user-approved
//!   2026-10-17**: switched the table's source to that same exhaustive
//!   dump against the pinned utf8proc instead, committed as
//!   `fold_table.rs`. Still pure Rust, still no utf8proc linked into
//!   the Rust side, still drops utf8proc from `HP2Dependencies.cmake`
//!   once `hp2-import` fully lands -- just a different source for the
//!   table's data, found necessary because `unicode-normalization`'s
//!   turned out to be stale relative to the pinned C library it must
//!   match.
//! - **`resolve_link_mode`'s non-`EXDEV` probe-failure fallback**
//!   (implemented; see [`plan`]). The real C++ `ResolveLinkMode`
//!   returns `"hardlink"` optimistically after *any* hard-link probe
//!   failure other than crossing filesystems, so on a filesystem that
//!   rejects hard links for some other reason (confirmed reachable on
//!   this machine: hard-linking a directory fails with `EPERM`, not
//!   `EXDEV`), planning would wrongly record "hardlink" and the real
//!   link would then fail at commit time -- a likely latent C++ bug.
//!   Flagged to the user; presented in plain language; **user-approved
//!   2026-10-17**: hardened instead of transcribed literally -- *any*
//!   probe failure, not just a cross-device one, now falls back to
//!   `"copy"` (or fails outright with the real OS error text if the
//!   caller explicitly required `Hardlink`). This is a deliberate
//!   behavior change, not a byte-for-byte port, at this one function.
//!   **Intended divergence:** `importer_parity`'s comparison against
//!   the real C++ importer will differ (copy vs. hardlink, or an error
//!   where C++ would silently succeed) on any filesystem where a hard
//!   link fails for a reason other than `EXDEV` -- expected, not a
//!   regression to chase down later.

pub mod archive;
pub mod classify;
pub mod collect;
pub mod files;
mod fold_table;
#[cfg(test)]
mod hash_tests {
    use hp2_file_hash::{hash, hash_bytes};
    use sha2::{Digest, Sha256};
    use std::path::Path;
    use crate::util::test_support::{TempDir, tempdir, write};

    /// Returns the owning `TempDir` alongside the file path: the caller
    /// must keep it alive for the duration of the test (just bind it to
    /// a variable) so `Drop` removes the scratch directory automatically
    /// at the end, including if an assertion panics first -- unlike the
    /// previous version of this helper, which required the caller to
    /// remember a manual `fs::remove_file` and would leak the file on an
    /// early panic.
    fn write_temp(name: &str, content: &[u8]) -> (TempDir, std::path::PathBuf) {
        let dir = tempdir();
        write(dir.path(), name, content);
        let path = dir.path().join(name);
        (dir, path)
    }

    /// Ground truth from a throwaway probe linked against the real macOS
    /// CommonCrypto branch of HP2PortableSha256.h.
    #[test]
    fn matches_commoncrypto_reference_vectors() {
        let cases: &[(&str, &[u8], &str)] = &[
            (
                "empty",
                b"",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "a",
                b"a",
                "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
            ),
            (
                "pangram",
                b"The quick brown fox jumps over the lazy dog",
                "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
            ),
            (
                "hp2",
                b"hp2",
                "fd37fed028b7eec670fbe8b261c9a49a9945279bbb72c228f6bb5ba570af2ab6",
            ),
        ];
        for (name, content, expected_hex) in cases {
            let (_dir, path) = write_temp(name, content);
            let (size, hex) = hash(&path).unwrap();
            assert_eq!(size, content.len() as u64, "size for {name}");
            assert_eq!(&hex, expected_hex, "digest for {name}");
            assert_eq!(&hash_bytes(content), expected_hex, "hash_bytes for {name}");
        }
    }


    #[test]
    fn spans_multiple_io_copy_buffer_boundaries() {
        // io::copy's internal buffer is a few KB; this exercises several
        // reads regardless of its exact size.
        let content = vec![0xABu8; 1024 * 1024 * 2 + 17];
        let (_dir, path) = write_temp("multi-chunk", &content);
        let (size, hex) = hash(&path).unwrap();
        assert_eq!(size, content.len() as u64);
        // Cross-check against a single-shot sha2 digest of the same bytes.
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let expected: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, expected);
    }

}
pub mod lower;
pub mod manifest;
pub mod output;
pub mod plan;
mod util;
