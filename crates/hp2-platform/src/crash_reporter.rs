//! Async-signal-safe crash reporter (POSIX). Port of the deleted
//! `Core/Src/HP2CrashReporter.cpp`, byte-for-byte in output.
//!
//! Installs handlers for SIGSEGV/SIGBUS/SIGILL/SIGFPE/SIGABRT plus an optional
//! wall-clock watchdog. On a fatal signal or watchdog expiry it writes
//! `<HP2_ARTIFACT_DIR>/crash-report.json` (or `./crash-report.json`) and an
//! `HP2_CRASH <reason_code> <path>` line to stderr, then re-raises so core
//! dumps and the exit status stay honest.
//!
//! After a fatal signal only async-signal-safe calls run (open/read/write/
//! lseek/close, sigaction, raise, _exit, backtrace into static storage). All
//! buffers are statics sized at compile time; nothing on the signal path
//! allocates, locks, formats through `std`, or can panic (every index is
//! bounds-checked by construction, never by a panicking slice operation).
//! Frame addresses are written as hex for offline symbolication with `atos`;
//! the report also records the faulting address and PC from the signal
//! context and the main image's runtime load base and path, so
//! `atos -o <image_path> -l <image_base> <frame>` resolves every frame
//! (file and line when the binary carries line tables) without guessing
//! the ASLR slide.
//!
//! Besides the JSON report, `log` writes a human-readable, timestamped
//! `crash-<UTC>-<pid>.log` into the player's `Logs` directory (message,
//! backtrace, version, engine-log tail) for fatal signals, Rust panics and
//! fatal errors reported through `report_fatal_error`.

mod log;

pub use log::{report_fatal_error, set_engine_log};
use std::cell::UnsafeCell;
use std::ffi::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

const MAX_FRAMES: usize = 64;
const LOG_TAIL_BYTES: usize = 16384;
const REPORT_CAPACITY: usize = 262_144;
const ARTIFACT_DIR_CAPACITY: usize = 1024;
const PROCESS_NAME_CAPACITY: usize = 128;
const LOG_HINT_CAPACITY: usize = 1024;
const REPORT_PATH_CAPACITY: usize = 1152;
const IMAGE_PATH_CAPACITY: usize = 1024;
const STR_SCAN_CAP: usize = 4096;
const REPORT_FILE_NAME: &[u8] = b"crash-report.json\0";
const USER_ROOT: &[u8] = b"/Library/Application Support/Harry Potter 2/User/";
const WATCHED_SIGNALS: [c_int; 5] = [
    libc::SIGSEGV,
    libc::SIGBUS,
    libc::SIGILL,
    libc::SIGFPE,
    libc::SIGABRT,
];

/// Configuration passed by the launcher (`hp2_game`) and `UCC`.
#[repr(C)]
pub struct Hp2CrashReporterConfig {
    /// Human-readable process label embedded in reports ("hp2_game", "UCC").
    pub process_name: *const c_char,
    /// Engine log filename under the user root (e.g. "HarryPotter2.log");
    /// its tail is embedded into every report.
    pub log_file_base: *const c_char,
    /// Optional wall-clock deadline from install time. 0 disables.
    pub watchdog_seconds: f64,
}

unsafe extern "C" {
    // <execinfo.h>; in libSystem on macOS and glibc on Linux.
    fn backtrace(buffer: *mut *mut c_void, size: c_int) -> c_int;
}

/// Fixed-size byte storage shared between the install path and the signal
/// handler. Access is unsynchronized by design, exactly like the C++ statics:
/// install writes happen before handlers are armed, and the handler path is
/// serialized by `REPORT_WRITTEN`.
struct Bytes<const N: usize>(UnsafeCell<[u8; N]>);

// SAFETY: see the type's documentation; accesses follow the original's
// install-then-arm ordering and the single-writer `REPORT_WRITTEN` latch.
unsafe impl<const N: usize> Sync for Bytes<N> {}

impl<const N: usize> Bytes<N> {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; N]))
    }

    fn ptr(&self) -> *mut u8 {
        self.0.get().cast()
    }

    /// Byte at `index`, or 0 past the end.
    fn at(&self, index: usize) -> u8 {
        if index < N {
            // SAFETY: in bounds; plain byte read of static storage.
            unsafe { *self.ptr().add(index) }
        } else {
            0
        }
    }

    fn set(&self, index: usize, value: u8) {
        if index < N {
            // SAFETY: in bounds; plain byte write of static storage.
            unsafe { *self.ptr().add(index) = value };
        }
    }

    /// Length up to the first NUL (capped like `StrLen`).
    fn c_len(&self) -> usize {
        // SAFETY: the storage is always NUL-terminated within N (writers
        // keep at least one trailing zero) or the scan stops at N.
        unsafe { str_len(self.ptr().cast(), N) }
    }
}

struct Report {
    buf: Bytes<REPORT_CAPACITY>,
    len: UnsafeCell<usize>,
}

// SAFETY: only touched by the latched single writer in `write_report`.
unsafe impl Sync for Report {}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static REPORT_WRITTEN: AtomicBool = AtomicBool::new(false);
static WATCHDOG_SECONDS: AtomicU64 = AtomicU64::new(0);
static INSTALL_TIME: AtomicU64 = AtomicU64::new(0);
static ARTIFACT_DIR: Bytes<ARTIFACT_DIR_CAPACITY> = Bytes::new();
static PROCESS_NAME: Bytes<PROCESS_NAME_CAPACITY> = Bytes::new();
static LOG_HINT: Bytes<LOG_HINT_CAPACITY> = Bytes::new();
static REPORT_PATH: Bytes<REPORT_PATH_CAPACITY> = Bytes::new();
static LOG_TAIL: Bytes<LOG_TAIL_BYTES> = Bytes::new();
/// Bytes of `LOG_TAIL` filled by the last report, and whether they are UTF-16LE (BOM seen).
static LOG_TAIL_LEN: AtomicUsize = AtomicUsize::new(0);
static LOG_TAIL_UTF16: AtomicBool = AtomicBool::new(false);
static REPORT: Report = Report {
    buf: Bytes::new(),
    len: UnsafeCell::new(0),
};

struct Frames(UnsafeCell<[*mut c_void; MAX_FRAMES]>);
// SAFETY: written only by the latched single writer in `write_report`.
unsafe impl Sync for Frames {}
static FRAMES: Frames = Frames(UnsafeCell::new([std::ptr::null_mut(); MAX_FRAMES]));
// Captured at fault time (signal context) and install time (dladdr); plain
// integer statics keep the handler path async-signal-safe.
static FAULT_ADDR: AtomicU64 = AtomicU64::new(0);
static FAULT_PC: AtomicU64 = AtomicU64::new(0);
static IMAGE_BASE: AtomicU64 = AtomicU64::new(0);
static IMAGE_PATH: Bytes<IMAGE_PATH_CAPACITY> = Bytes::new();

fn load_f64(cell: &AtomicU64) -> f64 {
    f64::from_bits(cell.load(Ordering::SeqCst))
}

fn store_f64(cell: &AtomicU64, value: f64) {
    cell.store(value.to_bits(), Ordering::SeqCst);
}

fn now_monotonic() -> f64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid out-pointer.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 * 1e-9
}

/// `StrLen`: 0 for null, else bytes up to the first NUL, capped at `cap`
/// (and never past `STR_SCAN_CAP`).
///
/// # Safety
/// `s` must be null or readable up to the first NUL or `cap` bytes.
unsafe fn str_len(s: *const c_char, cap: usize) -> usize {
    if s.is_null() {
        return 0;
    }
    let cap = cap.min(STR_SCAN_CAP);
    let mut n = 0;
    // SAFETY: caller guarantees readability up to NUL or `cap`.
    while n < cap && unsafe { *s.add(n) } != 0 {
        n += 1;
    }
    n
}

/// Copies `n` bytes of `src` into `dst` starting at `offset` (bounded by `dst`).
///
/// # Safety
/// `src` must be readable for `n` bytes.
unsafe fn copy_into<const N: usize>(dst: &Bytes<N>, offset: usize, src: *const u8, n: usize) {
    for i in 0..n {
        // SAFETY: caller guarantees `src` readable for `n` bytes.
        dst.set(offset + i, unsafe { *src.add(i) });
    }
}

/// Appends into the report; silently truncates at capacity.
///
/// # Safety
/// `text` must be readable for `n` bytes; caller is the latched writer.
unsafe fn append(text: *const u8, n: usize) {
    // SAFETY: latched single writer.
    let len = unsafe { &mut *REPORT.len.get() };
    if *len >= REPORT_CAPACITY - 1 {
        return;
    }
    let n = n.min(REPORT_CAPACITY - 1 - *len);
    // SAFETY: forwarded caller guarantee.
    unsafe { copy_into(&REPORT.buf, *len, text, n) };
    *len += n;
    REPORT.buf.set(*len, 0);
}

fn append_bytes(text: &[u8]) {
    // SAFETY: a slice is readable for its length.
    unsafe { append(text.as_ptr(), text.len()) };
}

/// `AppendC` of a NUL-terminated C string (StrLen-capped).
///
/// # Safety
/// `text` must be null or NUL-terminated within `STR_SCAN_CAP` bytes.
unsafe fn append_c(text: *const c_char) {
    // SAFETY: forwarded caller guarantee.
    unsafe { append(text.cast(), str_len(text, STR_SCAN_CAP)) };
}

fn append_u64(mut value: u64) {
    let mut buf = [0u8; 24];
    let mut p = buf.len() - 1;
    loop {
        p -= 1;
        buf[p] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let end = buf.len() - 1;
    append_bytes(&buf[p..end]);
}

fn append_hex(mut value: usize) {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [0u8; 20];
    let mut p = buf.len() - 1;
    loop {
        p -= 1;
        buf[p] = DIGITS[value & 0xF];
        value >>= 4;
        if value == 0 {
            break;
        }
    }
    append_bytes(b"0x");
    let end = buf.len() - 1;
    append_bytes(&buf[p..end]);
}

fn append_signal_name(signal: c_int) {
    match signal {
        libc::SIGSEGV => append_bytes(b"SIGSEGV"),
        libc::SIGBUS => append_bytes(b"SIGBUS"),
        libc::SIGILL => append_bytes(b"SIGILL"),
        libc::SIGFPE => append_bytes(b"SIGFPE"),
        libc::SIGABRT => append_bytes(b"SIGABRT"),
        _ => {
            append_bytes(b"SIG?");
            append_u64(signal as u64);
        }
    }
}

/// Reads up to `LOG_TAIL_BYTES` from the end of the hinted log file and
/// appends it as an escaped `log_tail` JSON string.
fn append_engine_log_tail() {
    if LOG_HINT.at(0) == 0 {
        return;
    }
    // SAFETY: LOG_HINT is NUL-terminated static storage.
    let fd = unsafe { libc::open(LOG_HINT.ptr().cast(), libc::O_RDONLY) };
    if fd < 0 {
        return;
    }
    // SAFETY: `fd` is an open descriptor.
    let size = unsafe { libc::lseek(fd, 0, libc::SEEK_END) };
    if size < 0 {
        // SAFETY: `fd` is an open descriptor.
        unsafe { libc::close(fd) };
        return;
    }
    // The engine log is UTF-16LE with a BOM; keep the tail on a code-unit boundary then.
    let mut bom = [0u8; 2];
    // SAFETY: `fd` is open and `bom` is a valid 2-byte destination.
    let utf16 = unsafe { libc::pread(fd, bom.as_mut_ptr().cast(), 2, 0) } == 2 && bom == [0xFF, 0xFE];
    LOG_TAIL_UTF16.store(utf16, Ordering::SeqCst);
    let tail = LOG_TAIL_BYTES as libc::off_t;
    let mut start = if size > tail { size - tail } else { 0 };
    if utf16 {
        start &= !1;
    }
    // SAFETY: `fd` is an open descriptor.
    unsafe { libc::lseek(fd, start, libc::SEEK_SET) };
    let mut total = 0usize;
    while total < LOG_TAIL_BYTES {
        // SAFETY: the destination range lies inside LOG_TAIL.
        let got = unsafe {
            libc::read(
                fd,
                LOG_TAIL.ptr().add(total).cast(),
                LOG_TAIL_BYTES - total,
            )
        };
        if got <= 0 {
            break;
        }
        total += got as usize;
    }
    // SAFETY: `fd` is an open descriptor.
    unsafe { libc::close(fd) };
    LOG_TAIL_LEN.store(total, Ordering::SeqCst);

    append_bytes(b",\"log_tail\":\"");
    for i in 0..total {
        let c = LOG_TAIL.at(i);
        if c == b'"' {
            append_bytes(b"\\\"");
        } else if c == b'\\' {
            append_bytes(b"\\\\");
        } else if c >= 0x20 && c != 0x7F {
            append_bytes(&[c]);
        } else if c == b'\n' || c == b'\r' || c == b'\t' {
            append_bytes(b" ");
        }
    }
    append_bytes(b"\"");
}

/// The interrupted PC from the signal's ucontext, or 0 when unavailable.
///
/// # Safety
/// `context` must be null or the kernel-provided `ucontext_t*` of a
/// synchronous fault.
unsafe fn context_pc(context: *mut c_void) -> usize {
    if context.is_null() {
        return 0;
    }
    unsafe {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            (*(*context.cast::<libc::ucontext_t>()).uc_mcontext).__ss.__pc as usize
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            (*context.cast::<libc::ucontext_t>()).uc_mcontext.pc as usize
        }
        #[cfg(not(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "linux", target_arch = "aarch64")
        )))]
        {
            0
        }
    }
}

/// Records the main image's runtime load base and path via `dladdr`, so
/// offline `atos -o <image_path> -l <image_base> <frame>` resolves the
/// report's hex frames without guessing the ASLR slide.
#[cfg(any(target_os = "macos", all(target_os = "linux", target_env = "gnu")))]
fn capture_image_info() {
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    // SAFETY: a function pointer is a valid dladdr subject; Dl_info is a
    // valid out-pointer.
    if unsafe { libc::dladdr(capture_image_info as *const c_void, &mut info) } != 0 {
        IMAGE_BASE.store(info.dli_fbase as u64, Ordering::SeqCst);
        if !info.dli_fname.is_null() {
            // SAFETY: dladdr yields a NUL-terminated path.
            let n = unsafe { str_len(info.dli_fname, STR_SCAN_CAP) }.min(IMAGE_PATH_CAPACITY - 1);
            // SAFETY: readable for `n` bytes.
            unsafe { copy_into(&IMAGE_PATH, 0, info.dli_fname.cast(), n) };
            IMAGE_PATH.set(n, 0);
        }
    }
}

#[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
fn capture_image_info() {}

fn write_fd(fd: c_int, bytes: *const u8, len: usize) {
    // SAFETY: caller passes a readable range; the result is ignored, as in
    // the original's unchecked write(2, ...) calls.
    unsafe { libc::write(fd, bytes.cast(), len) };
}

/// Builds and writes the report once per process (latched).
///
/// # Safety
/// Must not run concurrently with `install`'s first-call configuration
/// writes (the original's install-before-arm ordering guarantees this).
unsafe fn write_report(exit_reason: &[u8], reason_code: &[u8], signal: c_int) {
    if REPORT_WRITTEN.swap(true, Ordering::SeqCst) {
        return;
    }
    // SAFETY: latched single writer from here on.
    unsafe { *REPORT.len.get() = 0 };
    REPORT.buf.set(0, 0);

    append_bytes(b"{\"format\":\"hp2-crash\",\"schema_version\":1,\"exit_reason\":\"");
    append_bytes(exit_reason);
    append_bytes(b"\",\"reason_code\":\"");
    append_bytes(reason_code);
    append_bytes(b"\",\"process\":\"");
    if PROCESS_NAME.at(0) != 0 {
        // SAFETY: PROCESS_NAME is NUL-terminated static storage.
        unsafe { append_c(PROCESS_NAME.ptr().cast()) };
    } else {
        append_bytes(b"unknown");
    }
    append_bytes(b"\"");
    if signal > 0 {
        append_bytes(b",\"signal\":");
        append_u64(signal as u64);
        append_bytes(b",\"signal_name\":\"");
        append_signal_name(signal);
        append_bytes(b"\"");
    }
    let fault_addr = FAULT_ADDR.load(Ordering::SeqCst);
    if fault_addr != 0 {
        append_bytes(b",\"fault_addr\":\"");
        append_hex(fault_addr as usize);
        append_bytes(b"\"");
    }
    let fault_pc = FAULT_PC.load(Ordering::SeqCst);
    if fault_pc != 0 {
        append_bytes(b",\"fault_pc\":\"");
        append_hex(fault_pc as usize);
        append_bytes(b"\"");
    }
    let image_base = IMAGE_BASE.load(Ordering::SeqCst);
    if image_base != 0 {
        append_bytes(b",\"image_base\":\"");
        append_hex(image_base as usize);
        append_bytes(b"\",\"image_path\":\"");
        if IMAGE_PATH.at(0) != 0 {
            // SAFETY: IMAGE_PATH is NUL-terminated static storage.
            unsafe { append_c(IMAGE_PATH.ptr().cast()) };
        }
        append_bytes(b"\"");
    }
    append_engine_log_tail();

    let mut frame_count = 0usize;
    if WATCHED_SIGNALS.contains(&signal) {
        let frames: *mut *mut c_void = FRAMES.0.get().cast();
        // SAFETY: FRAMES holds MAX_FRAMES slots.
        let count = unsafe { backtrace(frames, MAX_FRAMES as c_int) };
        frame_count = count.max(0) as usize;
        append_bytes(b",\"frames\":[");
        for i in 0..count.max(0) as usize {
            if i != 0 {
                append_bytes(b",");
            }
            append_bytes(b"\"");
            // SAFETY: `i < count <= MAX_FRAMES`.
            append_hex(unsafe { *frames.add(i) } as usize);
            append_bytes(b"\"");
        }
        append_bytes(b"]");
    }
    append_bytes(b"}\n");

    if ARTIFACT_DIR.at(0) != 0 && REPORT_PATH.at(0) == 0 {
        let mut base = ARTIFACT_DIR.c_len();
        // SAFETY: ARTIFACT_DIR is readable for `base` bytes.
        unsafe { copy_into(&REPORT_PATH, 0, ARTIFACT_DIR.ptr(), base) };
        if base != 0 && REPORT_PATH.at(base - 1) != b'/' {
            REPORT_PATH.set(base, b'/');
            base += 1;
        }
        // SAFETY: a static slice is readable for its length.
        unsafe { copy_into(&REPORT_PATH, base, REPORT_FILE_NAME.as_ptr(), REPORT_FILE_NAME.len()) };
    } else if REPORT_PATH.at(0) == 0 {
        // SAFETY: a static slice is readable for its length.
        unsafe { copy_into(&REPORT_PATH, 0, REPORT_FILE_NAME.as_ptr(), REPORT_FILE_NAME.len()) };
    }

    // SAFETY: REPORT_PATH is NUL-terminated static storage.
    let fd = unsafe {
        libc::open(
            REPORT_PATH.ptr().cast(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_CLOEXEC,
            0o644 as libc::c_uint,
        )
    };
    if fd >= 0 {
        // SAFETY: latched single writer.
        let len = unsafe { *REPORT.len.get() };
        let mut off = 0usize;
        while off < len {
            // SAFETY: `off..len` lies inside REPORT.buf.
            let wrote = unsafe { libc::write(fd, REPORT.buf.ptr().add(off).cast(), len - off) };
            if wrote <= 0 {
                break;
            }
            off += wrote as usize;
        }
        // SAFETY: `fd` is an open descriptor.
        unsafe { libc::close(fd) };
    }

    const PREFIX: &[u8] = b"HP2_CRASH ";
    write_fd(2, PREFIX.as_ptr(), PREFIX.len());
    write_fd(2, reason_code.as_ptr(), reason_code.len());
    write_fd(2, b" ".as_ptr(), 1);
    write_fd(2, REPORT_PATH.ptr(), REPORT_PATH.c_len());
    write_fd(2, b"\n".as_ptr(), 1);

    // SAFETY: still the latched single writer; frames and log tail are in their statics.
    unsafe { log::write_signal_log(reason_code, signal, frame_count) };
}

extern "C" fn handler(signal: c_int, info: *mut libc::siginfo_t, context: *mut c_void) {
    if !info.is_null() {
        // SAFETY: the kernel passes a valid siginfo for a synchronous fault.
        FAULT_ADDR.store(unsafe { (*info).si_addr() } as u64, Ordering::SeqCst);
    }
    // SAFETY: null or the kernel-provided ucontext of the fault.
    FAULT_PC.store(unsafe { context_pc(context) } as u64, Ordering::SeqCst);
    // SAFETY: static reason strings.
    unsafe { write_report(b"signal", b"crash.signal", signal) };
    // Restore default and re-raise so core dumps/exit status stay honest.
    // SAFETY: zeroed sigaction is a valid SIG_DFL disposition; raise/_exit
    // are async-signal-safe.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = libc::SIG_DFL;
        libc::sigaction(signal, &sa, std::ptr::null_mut());
        libc::raise(signal);
        libc::_exit(128 + signal);
    }
}

fn watchdog_thread() {
    let deadline = load_f64(&INSTALL_TIME) + load_f64(&WATCHDOG_SECONDS);
    loop {
        let req = libc::timespec {
            tv_sec: 0,
            tv_nsec: 100 * 1000 * 1000,
        };
        // SAFETY: `req` is valid; the remainder pointer may be null.
        unsafe { libc::nanosleep(&req, std::ptr::null_mut()) };
        let now = now_monotonic();
        if load_f64(&WATCHDOG_SECONDS) <= 0.0 {
            continue;
        }
        if now >= deadline {
            break;
        }
    }
    // SAFETY: static reason strings; raise is always safe to call.
    unsafe {
        write_report(b"watchdog", b"crash.watchdog.timeout", 0);
        libc::raise(libc::SIGABRT);
    }
}

/// Installs the reporter. Configuration, install time and the watchdog thread
/// are set up once; signal dispositions are refreshed on every call (the
/// launcher installs early, then common Rust signal bootstrap replaces the
/// fatal handlers, and the post-bootstrap reinstall restores them).
///
/// # Safety
/// `config` must be null or point to a valid config whose string fields are
/// null or NUL-terminated.
pub unsafe fn install(config: *const Hp2CrashReporterConfig) {
    let first_install = !INSTALLED.load(Ordering::SeqCst);
    if first_install {
        INSTALLED.store(true, Ordering::SeqCst);

        // SAFETY: getenv returns null or a NUL-terminated string.
        let artifact_dir = unsafe { libc::getenv(c"HP2_ARTIFACT_DIR".as_ptr()) };
        // SAFETY: null or NUL-terminated.
        if !artifact_dir.is_null() && unsafe { *artifact_dir } != 0 {
            // SAFETY: NUL-terminated.
            let n = unsafe { str_len(artifact_dir, STR_SCAN_CAP) }.min(ARTIFACT_DIR_CAPACITY - 1);
            // SAFETY: readable for `n` bytes.
            unsafe { copy_into(&ARTIFACT_DIR, 0, artifact_dir.cast(), n) };
            ARTIFACT_DIR.set(n, 0);
        }

        // SAFETY: caller guarantees `config` is null or valid.
        if let Some(config) = unsafe { config.as_ref() } {
            // SAFETY: caller guarantees the string fields.
            let n = unsafe { str_len(config.process_name, STR_SCAN_CAP) }.min(PROCESS_NAME_CAPACITY - 1);
            // SAFETY: readable for `n` bytes (n == 0 when null).
            unsafe { copy_into(&PROCESS_NAME, 0, config.process_name.cast(), n) };
            PROCESS_NAME.set(n, 0);

            let watchdog = if config.watchdog_seconds < 0.0 {
                0.0
            } else if config.watchdog_seconds > 86400.0 {
                86400.0
            } else {
                config.watchdog_seconds
            };
            store_f64(&WATCHDOG_SECONDS, watchdog);

            // Default engine-log discovery: <user root>/<LogFileBase>, the
            // HP2Paths user-root layout.
            if LOG_HINT.at(0) == 0 && !config.log_file_base.is_null() {
                // SAFETY: getenv returns null or a NUL-terminated string.
                let home = unsafe { libc::getenv(c"HOME".as_ptr()) };
                if !home.is_null() {
                    // SAFETY: NUL-terminated strings.
                    let hn = unsafe { str_len(home, STR_SCAN_CAP) };
                    // SAFETY: caller guarantees `log_file_base`.
                    let bn = unsafe { str_len(config.log_file_base, STR_SCAN_CAP) };
                    let total = hn + USER_ROOT.len() + bn;
                    if total < LOG_HINT_CAPACITY - 1 {
                        // SAFETY: each source is readable for its length.
                        unsafe {
                            copy_into(&LOG_HINT, 0, home.cast(), hn);
                            copy_into(&LOG_HINT, hn, USER_ROOT.as_ptr(), USER_ROOT.len());
                            copy_into(&LOG_HINT, hn + USER_ROOT.len(), config.log_file_base.cast(), bn);
                        }
                        LOG_HINT.set(total, 0);
                    }
                }
            }
        }

        store_f64(&INSTALL_TIME, now_monotonic());
        capture_image_info();
        log::configure();
        log::install_panic_hook();
    }

    // SA_NODEFER: without it the signal stays blocked during `handler`, the
    // re-raise only queues it, and _exit discards it before the default
    // (core-dumping) action runs. SA_RESETHAND restores SIG_DFL on entry.
    // SAFETY: a zeroed sigaction with a valid handler and flags is valid.
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        let action: extern "C" fn(c_int, *mut libc::siginfo_t, *mut c_void) = handler;
        sa.sa_sigaction = action as usize;
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESETHAND | libc::SA_NODEFER;
        for signal in WATCHED_SIGNALS {
            libc::sigaction(signal, &sa, std::ptr::null_mut());
        }
    }

    if load_f64(&WATCHDOG_SECONDS) > 0.0 && first_install {
        // The original ignores pthread_create failure; so does this.
        let _ = std::thread::Builder::new().spawn(watchdog_thread);
    }

    if first_install {
        const INSTALLED_LINE: &[u8] = b"HP2_CRASH installed\n";
        write_fd(2, INSTALLED_LINE.as_ptr(), INSTALLED_LINE.len());
    }
}
