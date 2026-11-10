//! SHA-256 digests for files and in-memory bytes, encoded as lowercase hex.
//!
//! File hashing also returns the byte count. It uses `io::copy` to retry
//! interrupted reads (EINTR), rather than reporting a transient read error.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

/// Receives bytes from `io::copy`, which retries interrupted reads.
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

/// Returns the file size and its lowercase hexadecimal SHA-256 digest.
///
/// # Errors
///
/// Returns an error string containing the path if opening or reading fails.
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

/// Returns the lowercase hexadecimal SHA-256 digest of in-memory bytes.
pub fn hash_bytes(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex_digest(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{HashingSink, hash, hash_bytes, hex_digest};
    use sha2::{Digest, Sha256};
    use std::fs::{self, OpenOptions};
    use std::io::{self, Read, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let root = std::env::var_os("HP2_ARTIFACT_DIR")
                .expect("HP2_ARTIFACT_DIR must identify the fixture root");
            loop {
                let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
                let path = PathBuf::from(&root).join(format!("hash-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create fixture {}: {error}", path.display()),
                }
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove owned hash fixture directory");
        }
    }

    #[test]
    fn standard_sha256_vectors() {
        assert_eq!(
            hash_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hash_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn file_digest_and_count_match_memory_for_multiple_chunks() {
        let fixture = Fixture::new();
        let path = fixture.0.join("content");
        let content: Vec<u8> = (0..200_000).map(|i| (i % 251) as u8).collect();
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create owned fixture file");
        file.write_all(&content).expect("write fixture bytes");
        drop(file);
        assert_eq!(
            hash(&path).expect("hash fixture"),
            (content.len() as u64, hash_bytes(&content))
        );
    }

    #[test]
    fn missing_file_reports_open_failure() {
        let fixture = Fixture::new();
        let path = fixture.0.join("missing");
        assert_eq!(
            hash(&path),
            Err(format!("cannot hash {}: cannot open file", path.display()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_reports_read_failure() {
        let fixture = Fixture::new();
        assert_eq!(
            hash(&fixture.0),
            Err(format!("cannot hash {}: read error", fixture.0.display()))
        );
    }

    #[test]
    fn copy_retries_interrupted_reader() {
        struct InterruptedOnce<'a> {
            remaining: &'a [u8],
            interrupted: bool,
        }
        impl Read for InterruptedOnce<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                self.remaining.read(buf)
            }
        }
        let content = b"bytes after interruption";
        let mut reader = InterruptedOnce {
            remaining: content,
            interrupted: false,
        };
        let mut sink = HashingSink {
            hasher: Sha256::new(),
            total: 0,
        };
        let copied = io::copy(&mut reader, &mut sink).expect("copy retries interrupted read");
        assert!(reader.interrupted, "reader must inject an interruption");
        assert_eq!(copied, content.len() as u64);
        assert_eq!(sink.total, copied);
        assert_eq!(hex_digest(sink.hasher.finalize()), hash_bytes(content));
    }
}
