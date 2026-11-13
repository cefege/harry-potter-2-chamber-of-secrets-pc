//! Timestamped, human-readable crash logs for players.
//!
//! Complements the machine-readable `crash-report.json`: on a Rust panic, a fatal startup/run
//! error, or a fatal signal, `<app support>/Logs/crash-<UTC>-<pid>.log` is written with the
//! message, a backtrace, the version and the tail of the engine log. The directory is
//! `~/Library/Application Support/Harry Potter 2/Logs` on macOS and
//! `$XDG_DATA_HOME/harry-potter-2/Logs` (default `~/.local/share/...`) elsewhere;
//! `HP2_CRASH_LOG_DIR` overrides it. Old logs beyond `KEEP_LOGS` are pruned at install.
//!
//! The signal path follows the parent module's rules: static buffers, no allocation, no locks,
//! only async-signal-safe libc calls. Panics and fatal errors run in ordinary context and use std.

use super::{Bytes, FAULT_ADDR, FAULT_PC, FRAMES, IMAGE_BASE, IMAGE_PATH, LOG_HINT, LOG_TAIL, LOG_TAIL_LEN, LOG_TAIL_UTF16, MAX_FRAMES, PROCESS_NAME};
use std::ffi::{c_int, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

const LOGS_DIR_CAPACITY: usize = 1024;
const LOG_PATH_CAPACITY: usize = 1280;
const TEXT_CAPACITY: usize = 65_536;
/// Crash logs kept in the directory; older ones are deleted at install.
const KEEP_LOGS: usize = 20;
/// Logs one process may write, so a panic caught in a loop cannot fill the disk.
const MAX_LOGS: u32 = 8;
/// A SIGABRT this soon after a panic log is the panic's own abort, already covered by that log.
const PANIC_ABORT_WINDOW_SECONDS: i64 = 5;
const VERSION: &str = env!("CARGO_PKG_VERSION");

static LOGS_DIR: Bytes<LOGS_DIR_CAPACITY> = Bytes::new();
static LOG_PATH: Bytes<LOG_PATH_CAPACITY> = Bytes::new();
static TEXT: Bytes<TEXT_CAPACITY> = Bytes::new();
static WRITTEN: AtomicU32 = AtomicU32::new(0);
static LAST_PANIC_LOG: AtomicI64 = AtomicI64::new(i64::MIN);

/// The player-data root: `app_support` is `~/Library/Application Support/Harry Potter 2` on
/// macOS and `<XDG data home>/harry-potter-2` elsewhere (the `hp2-bootstrap-paths` layout).
pub(super) fn app_root(home: &Path, xdg_data_home: Option<&Path>, macos: bool) -> PathBuf {
    if macos {
        home.join("Library/Application Support/Harry Potter 2")
    } else {
        match xdg_data_home.filter(|path| path.is_absolute()) {
            Some(xdg) => xdg.join("harry-potter-2"),
            None => home.join(".local/share/harry-potter-2"),
        }
    }
}

fn default_logs_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HP2_CRASH_LOG_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = PathBuf::from(std::env::var_os("HOME").filter(|home| !home.is_empty())?);
    let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    Some(app_root(&home, xdg.as_deref(), cfg!(target_os = "macos")).join("Logs"))
}

/// Creates the log directory, records it for the signal handler and prunes old logs.
/// Returns the directory when logging is available.
pub(super) fn configure() -> Option<PathBuf> {
    let dir = default_logs_dir()?;
    let bytes = dir.as_os_str().as_bytes();
    if bytes.len() >= LOGS_DIR_CAPACITY - 1 || bytes.contains(&0) || std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    for (index, &byte) in bytes.iter().enumerate() {
        LOGS_DIR.set(index, byte);
    }
    LOGS_DIR.set(bytes.len(), 0);
    prune(&dir, KEEP_LOGS);
    Some(dir)
}

/// Deletes all but the newest `keep` `crash-*.log` files (names sort chronologically).
pub(super) fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut logs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().and_then(OsStr::to_str).is_some_and(|name| name.starts_with("crash-") && name.ends_with(".log"))
        })
        .collect();
    if logs.len() <= keep {
        return;
    }
    logs.sort();
    for old in &logs[..logs.len() - keep] {
        let _ = std::fs::remove_file(old);
    }
}

/// Points the tail embedded in crash reports at the engine's actual log file.
pub fn set_engine_log(path: &Path) {
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() >= super::LOG_HINT_CAPACITY - 1 || bytes.contains(&0) {
        return;
    }
    for (index, &byte) in bytes.iter().enumerate() {
        LOG_HINT.set(index, byte);
    }
    LOG_HINT.set(bytes.len(), 0);
}

// ---- timestamps ------------------------------------------------------------------------------

/// UTC civil date and time of a unix timestamp: (year, month, day, hour, minute, second).
fn civil(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, rem / 3_600, rem % 3_600 / 60, rem % 60)
}

fn put_digits(out: &mut [u8], mut value: u64) {
    for slot in out.iter_mut().rev() {
        *slot = b'0' + (value % 10) as u8;
        value /= 10;
    }
}

/// `YYYYMMDD-HHMMSS`, safe for file names.
fn stamp_file(secs: i64) -> [u8; 15] {
    let (y, mo, d, h, mi, s) = civil(secs);
    let mut out = *b"00000000-000000";
    put_digits(&mut out[0..4], y.clamp(0, 9999) as u64);
    put_digits(&mut out[4..6], u64::from(mo));
    put_digits(&mut out[6..8], u64::from(d));
    put_digits(&mut out[9..11], u64::from(h));
    put_digits(&mut out[11..13], u64::from(mi));
    put_digits(&mut out[13..15], u64::from(s));
    out
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
fn stamp_iso(secs: i64) -> [u8; 20] {
    let (y, mo, d, h, mi, s) = civil(secs);
    let mut out = *b"0000-00-00T00:00:00Z";
    put_digits(&mut out[0..4], y.clamp(0, 9999) as u64);
    put_digits(&mut out[5..7], u64::from(mo));
    put_digits(&mut out[8..10], u64::from(d));
    put_digits(&mut out[11..13], u64::from(h));
    put_digits(&mut out[14..16], u64::from(mi));
    put_digits(&mut out[17..19], u64::from(s));
    out
}

fn now_unix() -> i64 {
    // SAFETY: time(2) with a null out-pointer is async-signal-safe.
    i64::from(unsafe { libc::time(std::ptr::null_mut()) })
}

// ---- std-context logs: panics and fatal errors -----------------------------------------------

fn engine_log_tail() -> String {
    if LOG_HINT.at(0) == 0 {
        return String::new();
    }
    // SAFETY: LOG_HINT is NUL-terminated static storage.
    let path = Path::new(OsStr::from_bytes(unsafe { std::slice::from_raw_parts(LOG_HINT.ptr(), LOG_HINT.c_len()) }));
    tail_of_file(path, super::LOG_TAIL_BYTES)
}

/// Last `max` bytes of a text log, decoded when it is UTF-16LE (the engine log is).
pub(super) fn tail_of_file(path: &Path, max: usize) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else { return String::new() };
    let Ok(size) = file.seek(SeekFrom::End(0)) else { return String::new() };
    let mut bom = [0u8; 2];
    let utf16 = file.seek(SeekFrom::Start(0)).is_ok() && file.read_exact(&mut bom).is_ok() && bom == [0xFF, 0xFE];
    let mut start = size.saturating_sub(max as u64);
    if utf16 {
        start &= !1;
    }
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = if utf16 {
        let units: Vec<u16> = bytes.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    text.chars().filter(|&c| c == '\n' || c == '\t' || (!c.is_control() && c != '\u{feff}')).collect()
}

fn header(kind: &str, secs: i64) -> String {
    let process = if PROCESS_NAME.at(0) != 0 {
        // SAFETY: PROCESS_NAME is NUL-terminated static storage.
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(PROCESS_NAME.ptr(), PROCESS_NAME.c_len()) }).into_owned()
    } else {
        "unknown".to_owned()
    };
    format!(
        "HP2 crash log\nkind: {kind}\nversion: hp2 {VERSION} ({os} {arch})\nprocess: {process} (pid {pid})\ntime: {time}\nthread: {thread}\n",
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        pid = std::process::id(),
        time = String::from_utf8_lossy(&stamp_iso(secs)),
        thread = std::thread::current().name().unwrap_or("<unnamed>"),
    )
}

/// Renders a std-context log: `fields` are extra `key: value` header lines.
fn render(kind: &str, secs: i64, fields: &str, backtrace: &str) -> String {
    let mut text = header(kind, secs);
    text.push_str(fields);
    text.push_str("\nbacktrace:\n");
    text.push_str(backtrace);
    let tail = engine_log_tail();
    if !tail.is_empty() {
        text.push_str("\nrecent engine log (tail; may start mid-line):\n");
        text.push_str(&tail);
        if !tail.ends_with('\n') {
            text.push('\n');
        }
    }
    text
}

/// Writes `text` as a new `crash-<stamp>-<pid>[-n].log`; returns its path.
fn write_text_log(secs: i64, text: &str) -> Option<PathBuf> {
    if LOGS_DIR.at(0) == 0 || WRITTEN.fetch_add(1, Ordering::SeqCst) >= MAX_LOGS {
        return None;
    }
    // SAFETY: LOGS_DIR is NUL-terminated static storage.
    let dir = Path::new(OsStr::from_bytes(unsafe { std::slice::from_raw_parts(LOGS_DIR.ptr(), LOGS_DIR.c_len()) })).to_path_buf();
    let _ = std::fs::create_dir_all(&dir);
    let base = format!("crash-{}-{}", String::from_utf8_lossy(&stamp_file(secs)), std::process::id());
    for attempt in 0..16 {
        let name = if attempt == 0 { format!("{base}.log") } else { format!("{base}-{attempt}.log") };
        let path = dir.join(name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                use std::io::Write;
                let _ = file.write_all(text.as_bytes());
                let _ = file.sync_all();
                return Some(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

fn announce(path: Option<PathBuf>) {
    if let Some(path) = path {
        eprintln!("HP2_CRASH_LOG {}", path.display());
    }
}

/// Records a fatal (non-signal, non-panic) error: message, current backtrace, version, log tail.
pub fn report_fatal_error(message: &str) -> Option<PathBuf> {
    let secs = now_unix();
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let path = write_text_log(secs, &render("fatal error", secs, &format!("message: {message}\n"), &backtrace));
    announce(path.clone());
    path
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_owned())
}

/// Chains a panic hook in front of the current one that writes a crash log first.
pub(super) fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        thread_local! { static IN_HOOK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
        if !IN_HOOK.replace(true) {
            let secs = now_unix();
            let location = info.location().map_or_else(|| "<unknown>".to_owned(), ToString::to_string);
            let backtrace = std::backtrace::Backtrace::force_capture().to_string();
            let fields = format!("message: {}\nlocation: {location}\n", panic_message(info));
            let path = write_text_log(secs, &render("panic", secs, &fields, &backtrace));
            if path.is_some() {
                LAST_PANIC_LOG.store(secs, Ordering::SeqCst);
            }
            announce(path);
            IN_HOOK.set(false);
        }
        previous(info);
    }));
}

// ---- signal-context log ----------------------------------------------------------------------

struct Text {
    len: usize,
    last: u8,
}

impl Text {
    fn put(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.len < TEXT_CAPACITY - 1 {
                TEXT.set(self.len, byte);
                self.len += 1;
                self.last = byte;
            }
        }
    }

    fn num(&mut self, mut value: u64) {
        let mut digits = [0u8; 20];
        let mut at = digits.len();
        loop {
            at -= 1;
            digits[at] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.put(&digits[at..]);
    }

    fn hex(&mut self, mut value: usize) {
        let mut digits = [0u8; 16];
        let mut at = digits.len();
        loop {
            at -= 1;
            digits[at] = b"0123456789abcdef"[value & 0xF];
            value >>= 4;
            if value == 0 {
                break;
            }
        }
        self.put(b"0x");
        self.put(&digits[at..]);
    }

    /// Appends one Unicode scalar as UTF-8; control characters other than newline/tab are
    /// dropped and surrogates become `?`.
    fn emit(&mut self, ch: u32) {
        let mut buf = [0u8; 3];
        let n = match ch {
            0x0A | 0x09 | 0x20..=0x7E => {
                buf[0] = ch as u8;
                1
            }
            0..=0x1F | 0x7F => return,
            0x80..=0x7FF => {
                buf[0] = 0xC0 | (ch >> 6) as u8;
                buf[1] = 0x80 | (ch & 0x3F) as u8;
                2
            }
            0xD800..=0xDFFF => {
                buf[0] = b'?';
                1
            }
            _ => {
                buf[0] = 0xE0 | (ch >> 12) as u8;
                buf[1] = 0x80 | ((ch >> 6) & 0x3F) as u8;
                buf[2] = 0x80 | (ch & 0x3F) as u8;
                3
            }
        };
        self.put(&buf[..n]);
    }

    /// Engine-log tail as UTF-8 text (UTF-16LE is narrowed).
    fn tail(&mut self) {
        let total = LOG_TAIL_LEN.load(Ordering::SeqCst).min(super::LOG_TAIL_BYTES);
        if LOG_TAIL_UTF16.load(Ordering::SeqCst) {
            let mut i = 0;
            while i + 1 < total {
                self.emit(u32::from(LOG_TAIL.at(i)) | u32::from(LOG_TAIL.at(i + 1)) << 8);
                i += 2;
            }
        } else {
            // ASCII/UTF-8 log: printable ASCII and multibyte bytes pass through.
            for i in 0..total {
                let byte = LOG_TAIL.at(i);
                if byte >= 0x80 {
                    self.put(&[byte]);
                } else {
                    self.emit(u32::from(byte));
                }
            }
        }
        if total != 0 && self.last != b'\n' {
            self.put(b"\n");
        }
    }
}

fn signal_name(signal: c_int) -> &'static [u8] {
    match signal {
        libc::SIGSEGV => b"SIGSEGV",
        libc::SIGBUS => b"SIGBUS",
        libc::SIGILL => b"SIGILL",
        libc::SIGFPE => b"SIGFPE",
        libc::SIGABRT => b"SIGABRT",
        _ => b"SIG?",
    }
}

/// Writes the signal/watchdog crash log. Async-signal-safe; called by `write_report` after the
/// JSON report and the engine-log tail are in their static buffers.
///
/// # Safety
/// Latched single writer (`write_report`'s `REPORT_WRITTEN`); `frame_count <= MAX_FRAMES`
/// frames are valid in `FRAMES`.
pub(super) unsafe fn write_signal_log(reason_code: &[u8], signal: c_int, frame_count: usize) {
    if LOGS_DIR.at(0) == 0 {
        return;
    }
    let secs = now_unix();
    if signal == libc::SIGABRT && secs.saturating_sub(LAST_PANIC_LOG.load(Ordering::SeqCst)) <= PANIC_ABORT_WINDOW_SECONDS {
        return;
    }
    if WRITTEN.fetch_add(1, Ordering::SeqCst) >= MAX_LOGS {
        return;
    }

    let mut t = Text { len: 0, last: 0 };
    t.put(b"HP2 crash log\nkind: ");
    t.put(if signal > 0 { b"signal" } else { b"watchdog" });
    t.put(b"\nversion: hp2 ");
    t.put(VERSION.as_bytes());
    t.put(b" (");
    t.put(std::env::consts::OS.as_bytes());
    t.put(b" ");
    t.put(std::env::consts::ARCH.as_bytes());
    t.put(b")\nprocess: ");
    if PROCESS_NAME.at(0) != 0 {
        // SAFETY: PROCESS_NAME is NUL-terminated static storage.
        t.put(unsafe { std::slice::from_raw_parts(PROCESS_NAME.ptr(), PROCESS_NAME.c_len()) });
    } else {
        t.put(b"unknown");
    }
    // SAFETY: getpid(2) is async-signal-safe.
    let pid = unsafe { libc::getpid() };
    t.put(b" (pid ");
    t.num(pid as u64);
    t.put(b")\ntime: ");
    t.put(&stamp_iso(secs));
    t.put(b"\nreason: ");
    t.put(reason_code);
    if signal > 0 {
        t.put(b"\nmessage: fatal signal ");
        t.num(signal as u64);
        t.put(b" (");
        t.put(signal_name(signal));
        t.put(b")");
    } else {
        t.put(b"\nmessage: watchdog deadline exceeded");
    }
    let fault_addr = FAULT_ADDR.load(Ordering::SeqCst);
    if fault_addr != 0 {
        t.put(b"\nfault_addr: ");
        t.hex(fault_addr as usize);
    }
    let fault_pc = FAULT_PC.load(Ordering::SeqCst);
    if fault_pc != 0 {
        t.put(b"\nfault_pc: ");
        t.hex(fault_pc as usize);
    }
    let image_base = IMAGE_BASE.load(Ordering::SeqCst);
    if image_base != 0 {
        t.put(b"\nimage_base: ");
        t.hex(image_base as usize);
        t.put(b"\nimage_path: ");
        if IMAGE_PATH.at(0) != 0 {
            // SAFETY: IMAGE_PATH is NUL-terminated static storage.
            t.put(unsafe { std::slice::from_raw_parts(IMAGE_PATH.ptr(), IMAGE_PATH.c_len()) });
        }
    }
    t.put(b"\n\nbacktrace (return addresses; symbolicate with `atos -o <image_path> -l <image_base> <addr>`):\n");
    let frames: *mut *mut libc::c_void = FRAMES.0.get().cast();
    for i in 0..frame_count.min(MAX_FRAMES) {
        t.put(b"  #");
        t.num(i as u64);
        t.put(b" ");
        // SAFETY: `i < frame_count <= MAX_FRAMES`, all written by `backtrace`.
        t.hex(unsafe { *frames.add(i) } as usize);
        t.put(b"\n");
    }
    if LOG_TAIL_LEN.load(Ordering::SeqCst) != 0 {
        t.put(b"\nrecent engine log (tail; may start mid-line):\n");
        t.tail();
    }

    // <logs dir>/crash-<stamp>-<pid><suffix>
    let mut path_len = 0;
    let mut push = |bytes: &[u8], len: &mut usize| {
        for &byte in bytes {
            LOG_PATH.set(*len, byte);
            *len += 1;
        }
    };
    // SAFETY: LOGS_DIR is NUL-terminated static storage.
    push(unsafe { std::slice::from_raw_parts(LOGS_DIR.ptr(), LOGS_DIR.c_len()) }, &mut path_len);
    push(b"/crash-", &mut path_len);
    push(&stamp_file(secs), &mut path_len);
    push(b"-", &mut path_len);
    let mut digits = [0u8; 20];
    let mut at = digits.len();
    let mut value = pid as u64;
    loop {
        at -= 1;
        digits[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    push(&digits[at..], &mut path_len);
    let base_len = path_len;
    let mut fd = -1;
    for attempt in 0..16u8 {
        path_len = base_len;
        if attempt > 0 {
            push(b"-", &mut path_len);
            push(&[b'0' + attempt / 10 % 10, b'0' + attempt % 10][if attempt >= 10 { 0.. } else { 1.. }], &mut path_len);
        }
        push(b".log", &mut path_len);
        LOG_PATH.set(path_len, 0);
        // SAFETY: LOG_PATH is NUL-terminated static storage.
        fd = unsafe {
            libc::open(
                LOG_PATH.ptr().cast(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC,
                0o644 as libc::c_uint,
            )
        };
        if fd >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            break;
        }
    }
    if fd < 0 {
        return;
    }
    let mut off = 0;
    while off < t.len {
        // SAFETY: `off..t.len` lies inside TEXT.
        let wrote = unsafe { libc::write(fd, TEXT.ptr().add(off).cast(), t.len - off) };
        if wrote <= 0 {
            break;
        }
        off += wrote as usize;
    }
    // SAFETY: `fd` is an open descriptor.
    unsafe {
        libc::fsync(fd);
        libc::close(fd);
    }
    const PREFIX: &[u8] = b"HP2_CRASH_LOG ";
    // SAFETY: static and LOG_PATH ranges are readable.
    unsafe {
        libc::write(2, PREFIX.as_ptr().cast(), PREFIX.len());
        libc::write(2, LOG_PATH.ptr().cast(), path_len);
        libc::write(2, b"\n".as_ptr().cast(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn scratch(case: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hp2-crash-log-{case}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn crash_logs(dir: &Path) -> Vec<PathBuf> {
        let mut logs: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(Result::ok).map(|entry| entry.path()).collect())
            .unwrap_or_default();
        logs.retain(|path| path.file_name().and_then(OsStr::to_str).is_some_and(|name| name.starts_with("crash-")));
        logs.sort();
        logs
    }

    #[test]
    fn timestamps_are_utc_civil_time() {
        assert_eq!(&stamp_iso(0), b"1970-01-01T00:00:00Z");
        assert_eq!(&stamp_file(1_000_000_000), b"20010909-014640");
        assert_eq!(&stamp_iso(1_709_164_799), b"2024-02-28T23:59:59Z");
        assert_eq!(&stamp_iso(1_709_164_800), b"2024-02-29T00:00:00Z", "leap day");
        assert_eq!(&stamp_iso(1_709_164_800 + 86_400), b"2024-03-01T00:00:00Z");
        assert_eq!(&stamp_iso(1_794_528_896), b"2026-11-13T00:14:56Z");
    }

    #[test]
    fn logs_live_under_the_application_support_logs_directory() {
        let home = Path::new("/Users/someone");
        assert_eq!(
            app_root(home, None, true).join("Logs"),
            Path::new("/Users/someone/Library/Application Support/Harry Potter 2/Logs")
        );
        assert_eq!(
            app_root(home, Some(Path::new("/data")), false).join("Logs"),
            Path::new("/data/harry-potter-2/Logs")
        );
        assert_eq!(
            app_root(home, Some(Path::new("relative")), false).join("Logs"),
            Path::new("/Users/someone/.local/share/harry-potter-2/Logs"),
            "a relative XDG_DATA_HOME is ignored"
        );
        assert_eq!(app_root(home, None, false), Path::new("/Users/someone/.local/share/harry-potter-2"));
    }

    #[test]
    fn engine_log_tail_decodes_utf16_and_keeps_only_the_end() {
        let dir = scratch("tail");
        let path = dir.join("HarryPotter2.log");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "Log: first line\r\nLog: last line\r\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(tail_of_file(&path, 1 << 16), "Log: first line\nLog: last line\n");
        let tail = tail_of_file(&path, 22);
        assert!(tail.ends_with("last line\n") && !tail.contains("first"), "{tail:?}");
        std::fs::write(&path, b"plain ascii log\n").unwrap();
        assert_eq!(tail_of_file(&path, 1 << 16), "plain ascii log\n");
        assert_eq!(tail_of_file(&dir.join("missing"), 100), "");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_keeps_only_the_newest_crash_logs() {
        let dir = scratch("prune");
        for day in 1..=5 {
            std::fs::write(dir.join(format!("crash-2026110{day}-000000-1.log")), b"x").unwrap();
        }
        std::fs::write(dir.join("notes.txt"), b"keep").unwrap();
        prune(&dir, 2);
        let names: Vec<String> = crash_logs(&dir).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["crash-20261104-000000-1.log", "crash-20261105-000000-1.log"]);
        assert!(dir.join("notes.txt").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Re-runs this test binary as the crashing process; the parent test inspects what it left.
    fn run_child(case: &str, dir: &Path, engine_log: &Path) -> std::process::Output {
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_reporter::log::tests::crashing_child", "--nocapture", "--test-threads=1"])
            .env("HP2_CRASH_CHILD", case)
            .env("HP2_CRASH_LOG_DIR", dir)
            .env("HP2_CRASH_CHILD_ENGINE_LOG", engine_log)
            .env("HP2_ARTIFACT_DIR", dir.parent().unwrap())
            .env("RUST_BACKTRACE", "0")
            .output()
            .unwrap()
    }

    /// Body of the crashing child process; a no-op in an ordinary test run.
    #[test]
    fn crashing_child() {
        let Ok(case) = std::env::var("HP2_CRASH_CHILD") else { return };
        let config = super::super::Hp2CrashReporterConfig {
            process_name: c"hp2_test".as_ptr(),
            log_file_base: std::ptr::null(),
            watchdog_seconds: 0.0,
        };
        // SAFETY: the config's strings are static and NUL-terminated.
        unsafe { super::super::install(&config) };
        set_engine_log(Path::new(&std::env::var("HP2_CRASH_CHILD_ENGINE_LOG").unwrap()));
        match case.as_str() {
            "panic" => panic!("deliberate test panic"),
            "fatal" => {
                report_fatal_error("deliberate fatal error");
            }
            "signal" => {
                // SAFETY: raising a synchronous fatal signal is the point of this child.
                unsafe { libc::raise(libc::SIGSEGV) };
            }
            _ => unreachable!(),
        }
    }

    fn engine_log(dir: &Path) -> PathBuf {
        let path = dir.join("HarryPotter2.log");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "Log: engine started\r\nLog: loading Entry\r\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn panic_writes_a_timestamped_log_with_message_backtrace_version_and_log_tail() {
        let dir = scratch("panic");
        let logs = dir.join("Logs");
        let output = run_child("panic", &logs, &engine_log(&dir));
        assert!(!output.status.success());
        let files = crash_logs(&logs);
        assert_eq!(files.len(), 1, "{files:?}\n{}", String::from_utf8_lossy(&output.stderr));
        let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("crash-") && name.ends_with(".log") && name.len() > 25, "{name}");
        let text = std::fs::read_to_string(&files[0]).unwrap();
        for needle in [
            "HP2 crash log",
            "kind: panic",
            &format!("version: hp2 {VERSION}"),
            "process: hp2_test",
            "time: 20",
            "message: deliberate test panic",
            "location: ",
            "backtrace:",
            "crashing_child",
            "Log: loading Entry",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("HP2_CRASH_LOG "), "{stderr}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fatal_error_writes_a_log() {
        let dir = scratch("fatal");
        let logs = dir.join("Logs");
        let output = run_child("fatal", &logs, &engine_log(&dir));
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let files = crash_logs(&logs);
        assert_eq!(files.len(), 1);
        let text = std::fs::read_to_string(&files[0]).unwrap();
        assert!(text.contains("kind: fatal error") && text.contains("message: deliberate fatal error"), "{text}");
        assert!(text.contains("backtrace:") && text.contains("Log: engine started"), "{text}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fatal_signal_writes_a_log_alongside_the_json_report() {
        let dir = scratch("signal");
        let logs = dir.join("Logs");
        let output = run_child("signal", &logs, &engine_log(&dir));
        assert!(!output.status.success());
        let files = crash_logs(&logs);
        assert_eq!(files.len(), 1, "{files:?}\n{}", String::from_utf8_lossy(&output.stderr));
        let text = std::fs::read_to_string(&files[0]).unwrap();
        for needle in [
            "kind: signal",
            &format!("version: hp2 {VERSION}"),
            "process: hp2_test",
            "reason: crash.signal",
            "message: fatal signal 11 (SIGSEGV)",
            "backtrace (return addresses",
            "  #0 0x",
            "Log: engine started",
            "Log: loading Entry",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        assert!(dir.join("crash-report.json").exists(), "the JSON report is still written");
        let _ = std::fs::remove_dir_all(dir);
    }
}
