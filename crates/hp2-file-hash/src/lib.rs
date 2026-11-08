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

fn hex_digest(digest: impl IntoIterator<Item = u8>) -> String {
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        hex.push(char::from(b"0123456789abcdef"[(byte & 15) as usize]));
    }
    hex
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
    Ok((sink.total, hex_digest(sink.hasher.finalize())))
}

/// Replaces `HP2ImportPlan.cpp`'s `Generated`'s inline `CC_SHA256_*`
/// call sequence: hashes in-memory content directly (a just-built
/// `Default.ini`/`DefUser.ini` string, not a file on disk), using the
/// same SHA-256 and the same lowercase-hex formatting [`hash`] uses for
/// files -- the two call sites just differ in where the bytes come from.
pub fn hash_bytes(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex_digest(hasher.finalize())
}

