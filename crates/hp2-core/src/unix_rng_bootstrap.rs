//! Port of `appUnixBootstrapDiagnosticRngSeed` (`Core/Src/UnUnix.cpp`),
//! called by [`crate::unix_signal::initialize`] before `appInit`/class registration.
//! Reads `HP2_RNG_SEED`, decides whether it is absent/invalid/accepted using
//! [`crate::rng_seed`], and on acceptance seeds [`crate::rng_private`]
//! immediately. The requested/effective state is retained for lazy trace
//! initialization and is also published to an already initialized trace.
//! Publication does not initialize tracing or read its environment.

use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::rng_private;
use crate::rng_seed::{self, DiagnosticRngSeedBootstrap};

static SEEDED: AtomicBool = AtomicBool::new(false);
static REQUESTED: AtomicU32 = AtomicU32::new(0);
static EFFECTIVE: AtomicU32 = AtomicU32::new(0);

/// `getenv("HP2_RNG_SEED")`, decide, then either return, `exit(1)` after
/// printing the original diagnostic, or seed [`rng_private`] and publish the
/// state for [`diagnostic_rng_seed_state`]. `rand_max` is the caller's
/// `(unsigned)RAND_MAX`, kept as a native CRT constant rather than assumed.
///
/// An accepted bootstrap acquires the process trace mutex. The caller must not
/// hold that mutex in an active `with_process_state` or `with_process_state_raw`
/// closure; doing so would deadlock.
///
/// # Safety
/// None beyond the standard `getenv` contract (no concurrent `setenv`/
/// `putenv` on another thread; this runs during single-threaded bootstrap).
pub unsafe fn bootstrap_diagnostic_rng_seed(rand_max: u32) {
    // SAFETY: called during single-threaded platform bootstrap, before any
    // environment mutation.
    let text = unsafe { libc::getenv(c"HP2_RNG_SEED".as_ptr()) };
    // Matches `hp2_rs_core_rng_seed_prepare`'s `text.is_null() || text.read()==0`
    // precedent: an empty value is Absent,
    // not Invalid, matching the original `HP2PrepareDiagnosticRngSeed`.
    let outcome = if text.is_null() || unsafe { text.read() } == 0 {
        DiagnosticRngSeedBootstrap::Absent
    } else {
        // SAFETY: `getenv` returns a NUL-terminated string or null, handled above.
        match unsafe { rng_seed::parse(text.cast::<u8>(), rand_max) } {
            None => DiagnosticRngSeedBootstrap::Invalid,
            Some(requested) => {
                REQUESTED.store(requested, Ordering::SeqCst);
                EFFECTIVE.store(rng_seed::effective(requested, rand_max), Ordering::SeqCst);
                DiagnosticRngSeedBootstrap::Accepted
            }
        }
    };
    match outcome {
        DiagnosticRngSeedBootstrap::Absent => {}
        DiagnosticRngSeedBootstrap::Invalid => {
            let mut stderr = std::io::stderr().lock();
            let _ = writeln!(
                stderr,
                "Invalid HP2_RNG_SEED: expected an unsigned decimal value from 0 to {rand_max}"
            );
            drop(stderr);
            std::process::exit(1);
        }
        DiagnosticRngSeedBootstrap::Accepted => {
            SEEDED.store(true, Ordering::SeqCst);
            let effective = EFFECTIVE.load(Ordering::SeqCst);
            rng_private::srand(effective);
            crate::ansi_trace::publish_bootstrapped_diagnostic_seed(
                REQUESTED.load(Ordering::SeqCst), effective,
            );
        }
    }
}

/// Returns `Some((requested, effective))` after an accepted bootstrap.
/// Reading this state does not draw or reseed the generator.
pub fn diagnostic_rng_seed_state() -> Option<(u32, u32)> {
    if SEEDED.load(Ordering::SeqCst) {
        Some((
            REQUESTED.load(Ordering::SeqCst),
            EFFECTIVE.load(Ordering::SeqCst),
        ))
    } else {
        None
    }
}
