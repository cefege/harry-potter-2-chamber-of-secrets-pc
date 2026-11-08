//! Private Lehmer/Park-Miller generator backing oracle `appRand`/`appSRand`.
//!
//! On this platform's libc, `rand()` is the classic minimal-standard
//! (Park-Miller) generator with Schrage's method: `x' = 16807*x mod (2^31-1)`,
//! computed without overflow as `x' = 16807*(x mod 127773) - 2836*(x div
//! 127773)`, adding the modulus back when the result goes negative, and
//! substituting `123459876` for a zero state. Verified against this
//! platform's libc `rand()`: seeding with 12345 produces `207482415,
//! 1790989824, 2035175616, ...`, matching this module bit-for-bit.
//!
//! `appRand` used to call libc `rand()`/`srand()` directly. Those are a
//! single process-global generator shared by every piece of code in the
//! process, first-party or not. A macOS `CoreGraphics`/`CoreAnalytics`
//! telemetry callback (fired exactly once per process via `dispatch_once`,
//! triggered by first graphics use) was observed drawing from that same
//! global `rand()` state on a background queue at a wall-clock-dependent
//! point in the run, silently shifting HP2's RNG stream without being
//! reflected in HP2's own call-count/ordinal trace. Hosting the identical
//! generator algorithm privately here, with state nothing outside this
//! module can touch, removes that shared-state hazard entirely while
//! preserving this platform's exact output sequence for a given seed. See
//! `BD/reports/GameplayDeterminism.md`.
//!
//! Note: this is the algorithm this platform's `rand()` happens to use.
//! Linux glibc's `rand()` uses a different (TYPE_3 additive-feedback)
//! algorithm, so a build previously run against glibc's `rand()` will now
//! see a different `appRand` stream for the same seed; see the report.

use std::sync::atomic::{AtomicU32, Ordering};

const MODULUS: i64 = 0x7fff_ffff;
const MULTIPLIER: i64 = 16807;
const SCHRAGE_Q: i64 = 127_773;
const SCHRAGE_R: i64 = 2836;

/// Mirrors libc's unseeded default state of `1`; gameplay always seeds this
/// explicitly before reading from it (see `appUnixBootstrapDiagnosticRngSeed`
/// and `appPlatformInit` in `UnUnix.cpp`).
static STATE: AtomicU32 = AtomicU32::new(1);

/// Oracle `srand`: sets this generator's private state. Shares no state with
/// any libc RNG.
pub fn srand(seed: u32) {
    STATE.store(seed, Ordering::SeqCst);
}

/// Oracle `rand`: advances and returns this generator's private state via
/// the Park-Miller/Schrage recurrence.
pub fn rand() -> i32 {
    loop {
        let current = STATE.load(Ordering::SeqCst);
        let x = if current == 0 {
            123_459_876
        } else {
            i64::from(current)
        };
        let hi = x / SCHRAGE_Q;
        let lo = x % SCHRAGE_Q;
        let mut next = MULTIPLIER * lo - SCHRAGE_R * hi;
        if next < 0 {
            next += MODULUS;
        }
        let next_u32 = next as u32;
        if STATE
            .compare_exchange(current, next_u32, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return next as i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    // `srand`/`rand` share one process-global state (that is the entire point
    // of this module's test coverage), so tests in this module must not run
    // concurrently with each other.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn matches_platform_libc_rand_for_seed_12345() {
        let _guard = TEST_LOCK.lock().unwrap();
        srand(12345);
        let got: Vec<i32> = (0..5).map(|_| rand()).collect();
        assert_eq!(
            got,
            vec![
                207_482_415,
                1_790_989_824,
                2_035_175_616,
                77_048_696,
                24_794_531
            ]
        );
    }

    #[test]
    fn zero_seed_substitutes_123459876() {
        let _guard = TEST_LOCK.lock().unwrap();
        srand(0);
        let got = rand();
        srand(123_459_876);
        let expected = rand();
        assert_eq!(got, expected);
    }

    #[test]
    fn stays_within_rand_max() {
        let _guard = TEST_LOCK.lock().unwrap();
        srand(1);
        for _ in 0..1000 {
            assert!(rand() >= 0);
        }
    }

    // `RequireEndpointStreamMatchesOne` from `Tests/RngSeedTests.cpp`: both
    // zero-modulus diagnostic endpoints must reproduce the stream seeded by one.
    #[test]
    fn endpoint_seed_stream_matches_one() {
        const RAND_MAX: u32 = 0x7fff_ffff;
        let _guard = TEST_LOCK.lock().unwrap();
        for seed in [0, RAND_MAX] {
            srand(crate::rng_seed::effective(seed, RAND_MAX));
            let actual: Vec<i32> = (0..8).map(|_| rand()).collect();
            srand(1);
            let expected: Vec<i32> = (0..8).map(|_| rand()).collect();
            assert_eq!(
                actual, expected,
                "endpoint seed stream diverges from Rust normalization"
            );
        }
    }
}
