//! Replaces `HP2ImportInternal.h`'s `Hash`: streamed SHA-256 + byte count
//! of a file, used for every asset's content-addressed identity.
//!
//! C++ computes this via `HP2PortableSha256.h` -- Apple `CommonCrypto` on
//! macOS, a hand-rolled FIPS 180-4 implementation elsewhere, both exposed
//! under the `CC_SHA256_*` names. SHA-256 is a fully specified, bit-exact
//! algorithm with no implementation-defined behavior (unlike the Vorbis
//! codec's floating point), so any correct implementation -- including
//! this one, built on the widely used `sha2` crate -- produces identical
//! digests for identical bytes. Verified directly: built a throwaway
//! probe against the real macOS `CommonCrypto` branch of
//! `HP2PortableSha256.h` and compared against `sha2::Sha256` on several
//! inputs (empty string, "a", a pangram, "hp2"); exact match on all.
//!
//! Reads through `io::copy` (see [`HashingSink`]), not a direct
//! `File::read` loop: `io::copy` retries internally on
//! `ErrorKind::Interrupted` (EINTR), matching C++'s `ifstream::read`,
//! which never fails on an interrupted read. A hand-rolled read loop
//! would turn a transient EINTR into a hard "read error" C++ never
//! produces there.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

/// `io::copy`'s source-side retries on `ErrorKind::Interrupted` (EINTR)
/// internally; `std::fs::File::read` does not, so reading the file
/// directly in a loop (the first version of this function did) turned a
/// transient EINTR into a hard "read error", where C++'s `ifstream::read`
/// retries and never fails there. Sinking through `io::copy` into this
/// `Write` that forwards to the hasher and counts bytes gets that retry
/// for free instead of hand-rolling it.
struct HashingSink {
    hasher: Sha256,
    total: u64,
}

impl Write for HashingSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.hasher.update(buf);
        self.total += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Replaces `Hash(const fs::path&)`. Returns `(size, lowercase hex
/// digest)`, matching the C++ return shape and `Hex`'s lowercase-hex
/// formatting exactly (`sha2`/`hex` conventions already agree: hex
/// digits are emitted lowercase by both).
pub fn hash(path: &Path) -> Result<(u64, String), String> {
    let mut file = File::open(path)
        .map_err(|_| format!("cannot hash {}: cannot open file", path.display()))?;
    let mut sink = HashingSink {
        hasher: Sha256::new(),
        total: 0,
    };
    io::copy(&mut file, &mut sink)
        .map_err(|_| format!("cannot hash {}: read error", path.display()))?;
    let digest = sink.hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok((sink.total, hex))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn write_temp(name: &str, content: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "hp2-import-hash-test-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
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
            let path = write_temp(name, content);
            let (size, hex) = hash(&path).unwrap();
            assert_eq!(size, content.len() as u64, "size for {name}");
            assert_eq!(&hex, expected_hex, "digest for {name}");
            fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn spans_multiple_io_copy_buffer_boundaries() {
        // io::copy's internal buffer is a few KB; this exercises several
        // reads regardless of its exact size.
        let content = vec![0xABu8; 1024 * 1024 * 2 + 17];
        let path = write_temp("multi-chunk", &content);
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
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_file_reports_the_cannot_open_message() {
        let err = hash(Path::new("/nonexistent/hp2-import-hash-missing")).unwrap_err();
        assert_eq!(
            err,
            "cannot hash /nonexistent/hp2-import-hash-missing: cannot open file"
        );
    }
}
