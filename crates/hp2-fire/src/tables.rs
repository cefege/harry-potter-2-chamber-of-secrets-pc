//! Port of `InitTables()` minus the two engine calls that stay in C++: `appSin`
//! (passed in as `sine`) and `appRand` (passed in as `rng_seed`). The C++ shell
//! keeps the `Initialized` guard.

use crate::abi::FireTraceHooks;
use crate::globals::{FireGlobals, LIGHTPHASEBIAS};

/// `appRound` on GCC/clang is `static_cast<INT>(f)` (truncation), see
/// `Core/Inc/UnGnuG.h`. `as i32` truncates the same way for in-range values.
pub(crate) fn app_round(value: f32) -> i32 {
    value as i32
}

/// Fills the global tables from the C++-supplied engine results and reports
/// the init trace.
///
/// * `sine[t]` must equal `appSin((FLOAT)t/256.f * 6.2831853f)`, for `t` in `0..256`.
/// * `rng_seed` holds the 512 bytes `(BYTE)(appRand() & 255)` in draw order.
/// * `before` and `after` are `appGetRandTraceOrdinal()` read before the
///   `appRand` draws and after the init block.
#[allow(clippy::needless_range_loop)] // index loops mirror the C `for (t...)`
pub fn init_tables(
    g: &mut FireGlobals,
    sine: &[f64; 256],
    rng_seed: &[u8; 512],
    before: u64,
    after: u64,
) {
    for t in 0..256 {
        // C++: appRound(127.45f + 127.5f*appSin(...)). appSin is DOUBLE, so the sum is double and rounds to FLOAT only at appRound.
        g.phase_table[t] = app_round((127.45f32 as f64 + 127.5f64 * sine[t]) as f32) as u8;
    }
    for t in 0..256 {
        // C++: Clamp(PhaseTable[t] + LIGHTPHASEBIAS, 0, 255)
        let light = (g.phase_table[t] as i32 + LIGHTPHASEBIAS).clamp(0, 255);
        g.light_phase_table[t] = light as u8;
        // C++: (BYTE)(-128 + (SBYTE)PhaseTable[t])
        g.signed_phase_table[t] = (-128i32 + (g.phase_table[t] as i8) as i32) as u8;
    }
    g.speed_rand_arr.copy_from_slice(rng_seed);

    g.speed_rindex = 0;
    g.stale_rindex = 0;

    let fingerprint = fingerprint(g);
    (g.trace.init_tables)(before, after, fingerprint);
}

/// FNV-1a over SpeedRandArr(512), PhaseTable, SignedPhaseTable, LightPhaseTable,
/// in that order, matching the C++ `InitTables` fingerprint.
fn fingerprint(g: &FireGlobals) -> u64 {
    let mut hash: u64 = 1469598103934665603;
    let tables: [&[u8]; 4] = [
        &g.speed_rand_arr,
        &g.phase_table,
        &g.signed_phase_table,
        &g.light_phase_table,
    ];
    for table in tables {
        for &byte in table {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
    }
    hash
}

/// Hook-free variant used by unit tests that only need the table values.
pub fn init_tables_quiet(g: &mut FireGlobals, sine: &[f64; 256], rng_seed: &[u8; 512]) {
    let saved = g.trace;
    g.trace = FireTraceHooks::NOOP;
    init_tables(g, sine, rng_seed, 0, 0);
    g.trace = saved;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test holds the global lock for its whole body, so parallel test
    // threads cannot interleave writes to the shared RNG and tables.

    #[test]
    fn phase_tables_match_hand_values() {
        let mut g = crate::globals::lock();
        let mut sine = [0.0f64; 256];
        sine[0] = 1.0; // 127.45 + 127.5 = 254.95 -> 254
        sine[1] = -1.0; // 127.45 - 127.5 = -0.05 -> 0 (truncation)
        init_tables_quiet(&mut g, &sine, &[0u8; 512]);
        assert_eq!(g.phase_table[0], 254);
        assert_eq!(g.light_phase_table[0], 255); // 254 + 32 clamps to 255
        assert_eq!(g.signed_phase_table[0], 126); // -128 + (-2) = -130 -> 126
        assert_eq!(g.phase_table[1], 0);
        assert_eq!(g.light_phase_table[1], 32);
        assert_eq!(g.signed_phase_table[1], 128);
        assert_eq!(g.phase_table[2], 127); // sine 0 -> 127.45 -> 127
        assert_eq!(g.light_phase_table[2], 159);
        assert_eq!(g.signed_phase_table[2], 255); // -128 + 127 = -1 -> 255
    }

    #[test]
    fn speed_rand_walks_shift_register() {
        let mut g = crate::globals::lock();
        let mut seed = [0u8; 512];
        for (i, b) in seed.iter_mut().enumerate() {
            *b = i as u8;
        }
        init_tables_quiet(&mut g, &[0.0f64; 256], &seed);
        // idx 1: arr[(1+31)&63] = arr[32] = 32 ^ arr[1] (1) = 33
        assert_eq!(g.speed_rand(), 33);
        // idx 2: arr[33] = 33 ^ arr[2] (2) = 35
        assert_eq!(g.speed_rand(), 35);
        assert_eq!(g.speed_rindex, 2);
    }
}
