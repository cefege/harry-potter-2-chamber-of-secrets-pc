//! Runtime shared by every `extern "C"` export of the in-place port.
//!
//! * [`install_log_sink`] routes `log` records to the engine's `GLog` through a
//!   C callback; each line is UTF-8 and prefixed `HP2Rust: `.
//! * [`guard`] runs an export body under `catch_unwind` so a panic never
//!   unwinds into C++: it is logged as `HP2Rust: panic in <fn>: <msg>` and the
//!   export returns its error value instead.

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
}
