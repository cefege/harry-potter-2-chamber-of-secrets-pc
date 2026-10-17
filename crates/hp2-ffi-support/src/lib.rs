//! Runtime shared by every `extern "C"` export of the in-place port.
//!
//! * [`install_log_sink`] routes `log` records to the engine's `GLog` through a
//!   C callback; each line is UTF-8 and prefixed `HP2Rust: `.
//! * [`guard`] runs an export body under `catch_unwind` so a panic never
//!   unwinds into C++: it is logged as `HP2Rust: panic in <fn>: <msg>` and the
//!   export returns its error value instead.
//! * [`strerror`] formats an OS error code exactly as C++'s
//!   `std::error_code::message()`/`strerror(3)` does, for error text that
//!   must byte-match the C++ side (no trailing `" (os error N)"`, which
//!   `std::io::Error`'s `Display` appends and C++ never produces).
use std::any::Any;
use std::panic::{UnwindSafe, catch_unwind};

use log::{LevelFilter, Log, Metadata, Record};
use parking_lot::RwLock;

/// C++ receiver for one finished log line: UTF-8 bytes, not NUL-terminated.
pub type LogSink = unsafe extern "C" fn(text: *const u8, len: usize);

/// Prefix of every line Rust writes to the engine log.
pub const LOG_PREFIX: &str = "HP2Rust: ";

static SINK: RwLock<Option<LogSink>> = RwLock::new(None);
static BRIDGE: Bridge = Bridge;

struct Bridge;

impl Log for Bridge {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let sink = *SINK.read();
        if let Some(sink) = sink {
            let line = format!("{LOG_PREFIX}{}", record.args());
            // SAFETY: the sink contract is (pointer, length) of a live UTF-8
            // buffer for the duration of the call; `line` outlives it.
            unsafe { sink(line.as_ptr(), line.len()) };
        }
    }

    fn flush(&self) {}
}

/// Install (or replace) the engine log sink. `None` drops records.
///
/// The `log` facade accepts one logger per process; repeat calls only swap
/// the sink behind it.
pub fn install_log_sink(sink: Option<LogSink>) {
    *SINK.write() = sink;
    if log::set_logger(&BRIDGE).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

/// Run an export body, converting a panic into `on_panic`.
pub fn guard<R>(export: &str, on_panic: R, body: impl FnOnce() -> R + UnwindSafe) -> R {
    match catch_unwind(body) {
        Ok(value) => value,
        Err(payload) => {
            log::error!("panic in {export}: {}", panic_message(payload.as_ref()));
            on_panic
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(text) = payload.downcast_ref::<&str>() {
        text
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text
    } else {
        "<non-string panic payload>"
    }
}

/// `strerror(3)` / `std::error_code::message()` equivalent: the raw
/// OS-provided message text for an errno-style (POSIX) or `GetLastError`-
/// style (Windows) code, matching the C++ side byte for byte.
///
/// Built on `std::io::Error::from_raw_os_error(code).to_string()`, with
/// its trailing `" (os error {code})"` (which C++ never produces) peeled
/// off -- not a direct `libc::strerror` call, for two reasons:
///
/// 1. **Thread safety.** Plain `strerror` may return a pointer into a
///    shared static buffer on some libc implementations, not safe to
///    call concurrently. The importer runs on a worker thread while its
///    `Progress` atomics are polled from the launcher UI thread, and the
///    engine's own C++ threads may call `strerror`/`std::error_code`
///    concurrently too. `io::Error`'s `Display` uses the thread-safe
///    `strerror_r` on Unix internally.
/// 2. **Windows correctness.** `std::io::Error::raw_os_error()` on
///    Windows returns a Win32 `GetLastError` code, not a CRT `errno` --
///    `libc::strerror` would format it through the wrong table (e.g.
///    `ERROR_ACCESS_DENIED` = 5 reads as CRT `EIO`, not `EACCES` = 13).
///    `io::Error::from_raw_os_error` round-trips correctly on Windows
///    because it interprets its input as a Win32 code too, formatting it
///    via `FormatMessageW` -- matching what C++'s `std::error_code`
///    `system_category().message()` does there.
///
/// Verified on macOS: `io::Error::from_raw_os_error(code).to_string()`
/// with the suffix stripped equals `libc::strerror(code)` exactly for
/// every code checked (0, several real errno values, and an unknown
/// code) -- confirming the suffix is the only difference, not some
/// deeper text divergence between the two formatters.
pub fn strerror(code: i32) -> String {
    let text = std::io::Error::from_raw_os_error(code).to_string();
    let suffix = format!(" (os error {code})");
    match text.strip_suffix(suffix.as_str()) {
        Some(stripped) => stripped.to_string(),
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

    unsafe extern "C" fn capture(text: *const u8, len: usize) {
        // SAFETY: Bridge::log passes a live UTF-8 buffer of `len` bytes.
        let bytes = unsafe { std::slice::from_raw_parts(text, len) };
        LINES
            .lock()
            .push(String::from_utf8(bytes.to_vec()).unwrap());
    }

    #[test]
    fn panic_is_contained_and_logged_with_export_name() {
        install_log_sink(Some(capture));

        let value = guard("hp2_rs_test_export", -1, || -> i32 {
            panic!("bad index {}", 7)
        });
        assert_eq!(value, -1);
        assert_eq!(guard("hp2_rs_test_export", -1, || 5), 5);

        let lines = LINES.lock();
        assert_eq!(
            lines.as_slice(),
            ["HP2Rust: panic in hp2_rs_test_export: bad index 7"]
        );
    }

    #[test]
    #[cfg(unix)]
    fn strerror_matches_libc_text_without_errno_suffix() {
        // ENOENT: "No such file or directory" on every libc this project
        // targets. No " (os error N)" suffix, unlike io::Error's Display.
        // Unix-only: on Windows this code instead goes through
        // FormatMessageW with a different message ("The system cannot
        // find the file specified."), and raw_os_error() there returns a
        // Win32 GetLastError code, not a CRT errno, so the two platforms
        // aren't just differently worded -- they're different code
        // spaces entirely (see strerror's own doc comment). This is also
        // why hp2-ini's io::Error-sourced [Errno N] text in its own
        // "cannot read config" message will show a different number and
        // string than the C++ side's fopen/errno on Windows: a known,
        // not-yet-verified divergence there, unrelated to this function.
        let text = strerror(libc::ENOENT);
        assert_eq!(text, "No such file or directory");
        assert!(!text.contains("os error"));
    }

    #[test]
    #[cfg(windows)]
    fn strerror_formats_a_win32_code_via_format_message() {
        // ERROR_ACCESS_DENIED = 5. Confirms the Windows path actually
        // goes through FormatMessageW (distinct text from Unix's EIO,
        // which `libc::strerror(5)` would wrongly give for this same
        // numeric code -- exactly the bug strerror's doc describes).
        let text = strerror(5);
        assert_eq!(text, "Access is denied.");
        assert!(!text.contains("os error"));
    }

    #[test]
    fn strerror_never_leaves_the_os_error_suffix_attached() {
        // Platform-independent: whatever the message text is on this
        // platform, io::Error::Display's " (os error N)" suffix must
        // never survive strerror's stripping -- both for a real code
        // and for one no platform recognizes (which still round-trips
        // through io::Error::Display's "Unknown error: N (os error N)"
        // shape and must not leave that suffix attached either).
        assert!(!strerror(libc::ENOENT).contains("os error"));
        let text = strerror(999_999);
        assert!(!text.contains("os error"), "{text}");
    }
}
