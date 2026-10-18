//! Port of `Render/Src/UnRandom.cpp` (`FGlobalRandoms::Tick`).
//!
//! C++ owns the storage (`RandomBases`, `Randoms`, `RandomDeltas`, `LastTicks`)
//! and the engine RNG; this module receives them by reference and calls the
//! supplied `frand` closure in the same order as `appFrand()`.

pub const RAND_CYCLE: u32 = 16;
pub const N_RANDS: usize = 256;
pub const RAND_MASK: u32 = 255;

/// `Clamp(X, 0.f, 1.f)` from `UnTemplate.h`: `X<Min ? Min : (X<Max ? X : Max)`.
/// NaN falls through to `Max`, matching the C++ comparison chain.
#[inline]
fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x < 1.0 {
        x
    } else {
        1.0
    }
}

/// One `FGlobalRandoms::Tick(TimeSeconds)`.
pub fn tick(
    frand: &mut dyn FnMut() -> f32,
    time_seconds: f32,
    bases: &mut [f32; N_RANDS],
    randoms: &mut [f32; N_RANDS],
    deltas: &mut [f32; N_RANDS],
    last_ticks: &mut u32,
) {
    let server_ticks = (time_seconds as f64 * 35.0) as u32;

    // Regenerate all random bases for temporally discontinuous random numbers.
    for base in bases.iter_mut() {
        *base = frand();
    }

    let last = *last_ticks;
    let elapsed = server_ticks.wrapping_sub(last);

    // Update range of random numbers.
    if (elapsed.wrapping_mul(RAND_CYCLE) < N_RANDS as u32) && last != 0 {
        // No wraparound.
        let mut r = (server_ticks.wrapping_mul(RAND_CYCLE) & RAND_MASK) as usize;
        let m = (last.wrapping_mul(RAND_CYCLE) & RAND_MASK) as usize;
        let mut n = m;
        while n != r {
            deltas[n] = (bases[n] - randoms[n]) / (N_RANDS as u32 / RAND_CYCLE) as f32;
            n = (n + 1) & RAND_MASK as usize;
        }

        // Update all other randoms.
        let f = (elapsed & RAND_MASK) as f32;
        loop {
            // C++ `Randoms[r] += RandomDeltas[r] * f` is contracted to a fused
            // multiply-add by clang's default -ffp-contract=on (arm64); match it.
            randoms[r] = deltas[r].mul_add(f, randoms[r]);
            r = (r + 1) & RAND_MASK as usize;
            if r == m {
                break;
            }
        }
    } else {
        // Wraparound or first time.
        for i in 0..N_RANDS {
            randoms[i] = frand();
            deltas[i] = (bases[i] - randoms[i]) / (N_RANDS as u32 / RAND_CYCLE) as f32;
        }
    }

    for r in randoms.iter_mut() {
        *r = clamp01(*r);
    }

    *last_ticks = server_ticks;
}
