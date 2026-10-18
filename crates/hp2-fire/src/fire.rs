//! Port of the fire diffusion kernels in `UnFractal.cpp`: `CalculateFire`,
//! `CalcWrapFire`, `CalcSlowFire`, and the `SpeedRand` entry.
//!
//! Only the C path is ported. The `FIREASM` branches (`CalcWrapFireP2`,
//! `CalcSlowFireP2`, and the asm externs) are dead on arm64 and are removed.
//!
//! The diffusion kernels touch neither the RNG nor the trace hooks, so they
//! take no `FireTraceHooks`. `speed_rand` is the one entry here that does.

use crate::abi::FireTraceHooks;

/// `SpeedRand()` entry for C++ callers that have not been ported yet.
pub fn speed_rand(trace: FireTraceHooks) -> u8 {
    let mut g = crate::globals::lock();
    g.trace = trace;
    g.speed_rand()
}

/// Entry for `InitTables`. The C++ shell has already
/// done the engine calls: `sine` is `appSin` per entry and `rng_seed` is the
/// 512 `appRand` draws. `before` and `after` are the RNG trace ordinals.
pub fn init_tables(
    trace: FireTraceHooks,
    sine: &[f64; 256],
    rng_seed: &[u8; 512],
    before: u64,
    after: u64,
) {
    let mut g = crate::globals::lock();
    g.trace = trace;
    crate::tables::init_tables(&mut g, sine, rng_seed, before, after);
}

/// Copies the phase tables held in `FireGlobals` into arrays the C++ readers
/// still use (`PhaseTable`, `SignedPhaseTable`, `LightPhaseTable`).
pub fn read_phase_tables(
    phase: &mut [u8; 256],
    signed_phase: &mut [u8; 256],
    light_phase: &mut [u8; 256],
) {
    let g = crate::globals::lock();
    phase.copy_from_slice(&g.phase_table);
    signed_phase.copy_from_slice(&g.signed_phase_table);
    light_phase.copy_from_slice(&g.light_phase_table);
}

#[inline(always)]
fn px(b: &[u8], i: usize) -> usize {
    b[i] as usize
}

/// Fire table fill in `UFireTexture::PostLoad`.
/// C: `RenderTable[T] = (BYTE) Clamp( T/4.0 + 1.0 - (255-RenderHeat)/16.0, 0.0, 255.0 )`
/// for `T` in `0..1024`. The double operation order matches C.
#[allow(clippy::needless_range_loop)] // index loop mirrors the C `for (T...)`
pub fn fill_render_table(render_heat: u8, out: &mut [u8]) {
    assert!(out.len() >= 1024, "fill_render_table: need 1024 entries");
    let heat_term = (255 - render_heat as i32) as f64 / 16.0;
    for t in 0..1024usize {
        let v = t as f64 / 4.0 + 1.0 - heat_term;
        out[t] = v.clamp(0.0, 255.0) as u8;
    }
}

/// `CalculateFire`. Rows `0..ydim-2` are updated in place, top to bottom.
/// Edge columns drop the missing neighbour, so each sums three samples.
pub fn calculate_fire(bitmap: &mut [u8], render_table: &[u8], xdim: u32, ydim: u32) {
    let xd = xdim as usize;
    assert!(xd >= 2, "calculate_fire: xdim < 2");
    // C `DWORD Ydimension-2` wraps when ydim < 2, so the loop then runs away;
    // `wrapping_sub` keeps the same arithmetic, and the index panic is caught.
    let rows = (ydim as usize).wrapping_sub(2);
    for y in 0..rows {
        let this = y * xd;
        let below = this + xd;
        let lower = below + xd;

        // X=0: the left neighbour (BelowLine-1) is commented out in C.
        bitmap[this] = render_table[px(bitmap, below) + px(bitmap, below + 1) + px(bitmap, lower)];

        for x in 1..xd - 1 {
            bitmap[this + x] = render_table[px(bitmap, below + x)
                + px(bitmap, below + x - 1)
                + px(bitmap, below + x + 1)
                + px(bitmap, lower + x)];
        }

        // X=xd-1: the right neighbour (BelowLine+xd) is commented out in C.
        let r = xd - 1;
        bitmap[this + r] =
            render_table[px(bitmap, below + r) + px(bitmap, below + r - 1) + px(bitmap, lower + r)];
    }
}

/// One wrapping row of `CalcWrapFire`: `this` is written from the `below` and
/// `lower` rows, with horizontal neighbours wrapping at the frame edges.
/// `below` and `lower` are row start offsets. `lower` equals `this` only when
/// `ydim` is 2; that column is read before it is written, as in C.
fn wrap_row(
    bitmap: &mut [u8],
    render_table: &[u8],
    this: usize,
    below: usize,
    lower: usize,
    xd: usize,
) {
    // X=0: left neighbour wraps to BelowLine[xd-1].
    bitmap[this] = render_table[px(bitmap, below)
        + px(bitmap, below + xd - 1)
        + px(bitmap, below + 1)
        + px(bitmap, lower)];

    for x in 1..xd - 1 {
        bitmap[this + x] = render_table[px(bitmap, below + x)
            + px(bitmap, below + x - 1)
            + px(bitmap, below + x + 1)
            + px(bitmap, lower + x)];
    }

    // X=xd-1: right neighbour wraps to BelowLine[0].
    let r = xd - 1;
    bitmap[this + r] = render_table[px(bitmap, below + r)
        + px(bitmap, below + r - 1)
        + px(bitmap, below)
        + px(bitmap, lower + r)];
}

/// `CalcWrapFire`. The last two rows wrap vertically, reading row 0 and row 1
/// after they were updated by the main loop.
pub fn calc_wrap_fire(bitmap: &mut [u8], render_table: &[u8], xdim: u32, ydim: u32) {
    let xd = xdim as usize;
    let yd = ydim as usize;
    assert!(xd >= 2, "calc_wrap_fire: xdim < 2");
    assert!(yd >= 2, "calc_wrap_fire: ydim < 2");

    for y in 0..yd - 2 {
        wrap_row(bitmap, render_table, y * xd, (y + 1) * xd, (y + 2) * xd, xd);
    }
    // Line-before-last: lower line wraps to row 0.
    wrap_row(bitmap, render_table, (yd - 2) * xd, (yd - 1) * xd, 0, xd);
    // Last line: below wraps to row 0, lower wraps to row 1.
    wrap_row(bitmap, render_table, (yd - 1) * xd, 0, xd, xd);
}

/// `CalcSlowFire`. Each row is updated in place, so `this` is also the `below`
/// row. The last row's lower neighbour is the original row 0, which is kept in
/// a line buffer.
pub fn calc_slow_fire(bitmap: &mut [u8], render_table: &[u8], xdim: u32, ydim: u32) {
    let xd = xdim as usize;
    let yd = ydim as usize;
    assert!(xd >= 2, "calc_slow_fire: xdim < 2");
    assert!(yd >= 2, "calc_slow_fire: ydim < 2");
    // C `Line0Storage[256]`: a row wider than 256 overflows the stack buffer.
    assert!(
        xd <= 256,
        "calc_slow_fire: xdim > 256 overflows Line0Storage"
    );

    // C copies (xdim>>2) DWORDs, so only a multiple-of-4 width copies whole
    // bytes. Any tail bytes are left zero here, where C reads uninitialized stack.
    let mut line0 = [0u8; 256];
    let copy = 4 * (xd >> 2);
    line0[..copy].copy_from_slice(&bitmap[..copy]);

    for y in 0..yd - 1 {
        let this = y * xd;
        let lower = this + xd;

        // Below == This: reads see this row's earlier updates, as in C.
        bitmap[this] = render_table[px(bitmap, this)
            + px(bitmap, this + xd - 1)
            + px(bitmap, this + 1)
            + px(bitmap, lower)];

        for x in 1..xd - 1 {
            bitmap[this + x] = render_table[px(bitmap, this + x)
                + px(bitmap, this + x - 1)
                + px(bitmap, this + x + 1)
                + px(bitmap, lower + x)];
        }

        // Right neighbour wraps to Below[0], which is already updated.
        let r = xd - 1;
        bitmap[this + r] = render_table[px(bitmap, this + r)
            + px(bitmap, this + r - 1)
            + px(bitmap, this)
            + px(bitmap, lower + r)];
    }

    // Last line: lower line is the saved original row 0.
    let this = (yd - 1) * xd;
    bitmap[this] = render_table
        [px(bitmap, this) + px(bitmap, this + xd - 1) + px(bitmap, this + 1) + px(&line0, 0)];

    for x in 1..xd - 1 {
        bitmap[this + x] = render_table[px(bitmap, this + x)
            + px(bitmap, this + x - 1)
            + px(bitmap, this + x + 1)
            + px(&line0, x)];
    }

    let r = xd - 1;
    bitmap[this + r] = render_table
        [px(bitmap, this + r) + px(bitmap, this + r - 1) + px(bitmap, this) + px(&line0, r)];
}
