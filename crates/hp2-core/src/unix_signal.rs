//! Process signal ownership for the Mac/Unix runtime, replacing
//! `__Context::StaticInit` / `__Context::HandleSignal` (`Core/Src/UnUnix.cpp`).
//!
//! Policy:
//! - The first `SIGHUP`/`SIGQUIT`/`SIGTERM` prints its diagnostic, requests a
//!   graceful exit through the Rust-owned flag, and returns normally.
//! - Later exit signals that reach the handler, and every fatal signal
//!   (`SIGILL`/`SIGTRAP`/`SIGIOT`/`SIGBUS`/`SIGFPE`/`SIGSEGV`), prints its
//!   diagnostic and terminates the process with `_exit(1)`. Nothing unwinds or
//!   longjmps out of the handler.
//! - `SA_RESETHAND` leaves a repeated same signal on the OS default disposition.
//!
//! The handler uses only lock-free atomics, `write(2)` of static bytes, and
//! `_exit(2)`.

use std::os::raw::c_int;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::unix_rng_bootstrap;

/// Process-wide graceful-exit request, polled by the engine main loop.
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Cumulative count of exit signals (`SIGHUP`/`SIGQUIT`/`SIGTERM`) received.
static EXIT_SIGNALS: AtomicU32 = AtomicU32::new(0);

/// Returns whether a graceful exit has been requested.
pub fn exit_requested() -> bool {
    EXIT_REQUESTED.load(Ordering::Relaxed)
}

/// Sets the graceful-exit request. A plain request flag needs no ordering
/// with other memory.
pub fn set_exit_requested(value: bool) {
    EXIT_REQUESTED.store(value, Ordering::Relaxed);
}

/// Performs the early diagnostic RNG bootstrap for `rand_max`, then installs
/// this crate's signal policy.
///
/// # Safety
/// Must run during single-threaded platform bootstrap, before any concurrent
/// environment mutation (the RNG bootstrap reads `HP2_RNG_SEED`).
pub unsafe fn initialize(rand_max: u32) {
    // SAFETY: the caller guarantees single-threaded bootstrap.
    unsafe { unix_rng_bootstrap::bootstrap_diagnostic_rng_seed(rand_max) };
    install_signal_handlers();
}

/// Installs `handle_signal` for `SIGHUP`, `SIGQUIT`, `SIGILL`, `SIGTRAP`,
/// `SIGIOT`, `SIGBUS`, `SIGFPE`, `SIGSEGV`, `SIGTERM` (in that order, each with
/// `SA_RESETHAND` and an empty mask), then sets `SIGPIPE` to `SIG_IGN`.
fn install_signal_handlers() {
    const SIGNALS: [c_int; 9] = [
        libc::SIGHUP,
        libc::SIGQUIT,
        libc::SIGILL,
        libc::SIGTRAP,
        libc::SIGIOT,
        libc::SIGBUS,
        libc::SIGFPE,
        libc::SIGSEGV,
        libc::SIGTERM,
    ];
    let handler: extern "C" fn(c_int) = handle_signal;
    for &signal in &SIGNALS {
        // SAFETY: a plain, fully-initialized local `sigaction`; `handler` is a
        // process-lifetime `extern "C"` function, and a null old-disposition
        // out-param is always sound.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handler as libc::sighandler_t;
            libc::sigemptyset(&mut action.sa_mask);
            action.sa_flags = libc::SA_RESETHAND;
            libc::sigaction(signal, &action, null_mut());
        }
    }
    // SAFETY: `SIG_IGN` is a valid disposition constant, not a function
    // pointer dereference.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

extern "C" fn handle_signal(signal: c_int) {
    match signal {
        libc::SIGHUP => exit_signal(b"Signal: SIGHUP [hangup]\n"),
        libc::SIGQUIT => exit_signal(b"Signal: SIGQUIT [quit]\n"),
        libc::SIGTERM => exit_signal(b"Signal: SIGTERM [terminate]\n"),
        libc::SIGILL => fatal_signal(b"Signal: SIGILL [illegal instruction]\n"),
        libc::SIGTRAP => fatal_signal(b"Signal: SIGTRAP [trap]\n"),
        libc::SIGIOT => fatal_signal(b"Signal: SIGIOT [iot trap]\n"),
        libc::SIGBUS => fatal_signal(b"Signal: SIGBUS [bus error]\n"),
        libc::SIGFPE => fatal_signal(b"Signal: SIGFPE [floating point exception]\n"),
        libc::SIGSEGV => fatal_signal(b"Signal: SIGSEGV [segmentation fault]\n"),
        _ => {}
    }
}

/// First exit signal requests a graceful exit and returns; any later exit
/// signal terminates immediately.
fn exit_signal(diagnostic: &'static [u8]) {
    write_diagnostic(diagnostic);
    if EXIT_SIGNALS.fetch_add(1, Ordering::Relaxed) != 0 {
        exit_immediately();
    }
    write_diagnostic(b"Requesting Exit.\n");
    EXIT_REQUESTED.store(true, Ordering::Relaxed);
}

fn fatal_signal(diagnostic: &'static [u8]) -> ! {
    write_diagnostic(diagnostic);
    exit_immediately()
}

fn exit_immediately() -> ! {
    // SAFETY: `_exit` is async-signal-safe and never returns.
    unsafe { libc::_exit(1) }
}

/// Best-effort, single `write(2)` to stdout. The result is ignored: a signal
/// handler must not retry or report failures.
fn write_diagnostic(bytes: &'static [u8]) {
    // SAFETY: `write` is async-signal-safe; the pointer and length describe a
    // static byte slice.
    let _ = unsafe { libc::write(libc::STDOUT_FILENO, bytes.as_ptr().cast(), bytes.len()) };
}
