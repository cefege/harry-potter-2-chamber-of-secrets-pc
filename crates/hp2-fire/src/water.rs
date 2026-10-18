//! Water slice of `UnFractal.cpp`: `UWaterTexture::CalculateWater`, `WaterRedrawDrops`,
//! `AddDrop`, `DeleteDrops`, `WaterPaint`, `TouchTexture` and `UFireTexture::TempDrawSpark`.
//!
//! Line-by-line port of the C path. Operation order, read-after-write aliasing and the
//! order of `SpeedRand` calls are preserved. Every entry takes the trace hooks by value
//! and installs them in the globals before any RNG use.

use crate::abi::{FireDrop, FireTraceHooks};
use crate::globals::{self, FireGlobals};

const MAX_DROPS: i32 = 256;
const DEL_RANGE: i32 = 12;
const MOUSE_LEFT: u32 = 0x0001;
const MOUSE_RIGHT: u32 = 0x0002;

// `EDrop` (UnFractal.h).
const DROP_FIXED_DEPTH: u8 = 0;
const DROP_PHASE_SPOT: u8 = 1;
const DROP_SHALLOW_SPOT: u8 = 2;
const DROP_HALF_AMPL: u8 = 3;
const DROP_RANDOM_MOVER: u8 = 4;
const DROP_FIXED_RANDOM_SPOT: u8 = 5;
const DROP_WHIRLY_THING: u8 = 6;
const DROP_BIG_WHIRLY: u8 = 7;
const DROP_HORIZONTAL_LINE: u8 = 8;
const DROP_VERTICAL_LINE: u8 = 9;
const DROP_DIAGONAL_LINE1: u8 = 10;
const DROP_DIAGONAL_LINE2: u8 = 11;
const DROP_HORIZONTAL_OSC: u8 = 12;
const DROP_VERTICAL_OSC: u8 = 13;
const DROP_DIAGONAL_OSC1: u8 = 14;
const DROP_DIAGONAL_OSC2: u8 = 15;
const DROP_RAIN_DROPS: u8 = 16;
const DROP_AREA_CLAMP: u8 = 17;
const DROP_LEAKY_TAP: u8 = 18;
const DROP_DRIPPY_TAP: u8 = 19;

// Internal drop types (UnFractal.cpp).
const DROP_WHIRLY_BACK: u8 = 64;
const DROP_BIG_WHIRLY_BACK: u8 = 65;

/// Scalar members of `UWaterTexture` / `UFractalTexture` / `UTexture` read or written by the
/// water routines. `BYTE` members are `u8`, `INT` members are `i32`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireWaterState {
    pub u_size: i32,
    pub v_size: i32,
    pub u_bits: u8,
    pub u_mask: i32,
    pub v_mask: i32,
    pub global_phase: i32,
    pub draw_phase: u8,
    pub num_drops: i32,
    pub drop_type: u8,
    pub wave_parity: u8,
    pub fx_frequency: u8,
    pub fx_phase: u8,
    pub fx_size: u8,
    pub fx_depth: u8,
}

/// Geometry shared by the drop routines.
#[derive(Clone, Copy)]
struct Masks {
    /// `XSize = USize/2`: offset of `WaveFieldB` inside the source-field buffer.
    xsize: usize,
    /// `U2Mask = UMask >> 1` (BYTE), held as the int it promotes to.
    u2: i32,
    /// `V2Mask = VMask >> 1` (BYTE).
    v2: i32,
    ubits: u32,
}

/// Writes `WaveFieldA[idx] = a; WaveFieldB[idx] = b;` in that order.
#[inline]
fn put_ab(water: &mut [u8], xsize: usize, idx: usize, a: u8, b: u8) {
    water[idx] = a;
    water[xsize + idx] = b;
}

// ---------------------------------------------------------------------------
// CalculateWater (interpolating version).
// ---------------------------------------------------------------------------

struct WaterCells<'a> {
    water: &'a mut [u8],
    bitmap: &'a mut [u8],
    render: &'a [u8],
    wave: &'a [u8],
}

#[inline]
fn at(i: i64) -> usize {
    i as usize
}

impl WaterCells<'_> {
    /// `Output4Pix` macro. `src` is `[A, C, E, G, B, D, F, H]` (macro argument order) and
    /// `dst` is `[Dest1, Dest2, Dest3, Dest4]`. Operands are element indices, read at the
    /// same points where the macro re-evaluates them (after the `*DestCell` write).
    fn output4pix(&mut self, cell: i64, src: [i64; 8], dst: [i64; 4]) {
        let [a, c, e, gg, b, d, f, h] = src;
        let sum = i32::from(self.water[at(e)])
            + i32::from(self.water[at(gg)])
            + i32::from(self.water[at(f)])
            + i32::from(self.water[at(h)]);
        let cur = i32::from(self.water[at(cell)]);
        self.water[at(cell)] = self.wave[(512 + sum - (cur << 1)) as usize];

        let ea = i32::from(self.water[at(e)]) - i32::from(self.water[at(a)]);
        let fb = i32::from(self.water[at(f)]) - i32::from(self.water[at(b)]);
        let gc = i32::from(self.water[at(gg)]) - i32::from(self.water[at(c)]);
        let hd = i32::from(self.water[at(h)]) - i32::from(self.water[at(d)]);

        self.bitmap[at(dst[1])] = self.render[(512 + gc + hd) as usize];
        self.bitmap[at(dst[2])] = self.render[(512 + fb + hd) as usize];
        // C `/` truncates toward zero, as does Rust `/`.
        self.bitmap[at(dst[0])] = self.render[(512 + (fb + hd + ea + gc) / 2) as usize];
        self.bitmap[at(dst[3])] = self.render[(512 + hd + hd) as usize];
    }
}

/// EVEN water: source = cells on ODD lines, destin = EVEN lines.
fn even(c: &mut WaterCells<'_>, x: i64, yd: i64, t: i64) {
    // Upper row: 2 wrap up+left at beginning, n-2 all wrap up.
    let mut dc: i64 = 0;
    let mut p: i64 = 0;
    c.output4pix(
        dc,
        [
            dc + t - 3,
            dc + t - 2,
            dc + t - 1,
            dc + t - x,
            dc - 3 + 2 * x,
            dc - 2 + 2 * x,
            dc - 1 + 2 * x,
            dc + x,
        ],
        [p - 1 + 2 * t, p - 2 * x + 2 * t, p - 1 + 2 * x, p],
    );

    dc += 1;
    p += 2;
    c.output4pix(
        dc,
        [
            dc + t - 3,
            dc + t - 2,
            dc + t - 1 - x,
            dc + t - x,
            dc - 3 + 2 * x,
            dc - 2 + 2 * x,
            dc - 1 + x,
            dc + x,
        ],
        [p - 1 - 2 * x + 2 * t, p - 2 * x + 2 * t, p - 1, p],
    );

    dc += 1;
    p += 2;
    c.output4pix(
        dc,
        [
            dc + t - 3,
            dc + t - 2 - x,
            dc + t - 1 - x,
            dc + t - x,
            dc - 3 + 2 * x,
            dc - 2 + x,
            dc - 1 + x,
            dc + x,
        ],
        [p - 1 - 2 * x + 2 * t, p - 2 * x + 2 * t, p - 1, p],
    );

    // Total Xdimension-1 cells.
    for _ in 3..x {
        dc += 1;
        p += 2;
        c.output4pix(
            dc,
            [
                dc + t - 3 - x,
                dc + t - 2 - x,
                dc + t - 1 - x,
                dc + t - x,
                dc - 3 + x,
                dc - 2 + x,
                dc - 1 + x,
                dc + x,
            ],
            [p - 1 - 2 * x + 2 * t, p - 2 * x + 2 * t, p - 1, p],
        );
    }

    // Remaining rows: first 2 wrap, rest always works the same way.
    for y in 1..yd {
        let mut dc = y * 2 * x;
        let mut p = y * 4 * x;

        c.output4pix(
            dc,
            [
                dc - 3,
                dc - 2,
                dc - 1,
                dc - x,
                dc - 3 + 2 * x,
                dc - 2 + 2 * x,
                dc - 1 + 2 * x,
                dc + x,
            ],
            [p - 1, p - 2 * x, p - 1 + 2 * x, p],
        );

        dc += 1;
        p += 2;
        c.output4pix(
            dc,
            [
                dc - 3,
                dc - 2,
                dc - 1 - x,
                dc - x,
                dc - 3 + 2 * x,
                dc - 2 + 2 * x,
                dc - 1 + x,
                dc + x,
            ],
            [p - 1 - 2 * x, p - 2 * x, p - 1, p],
        );

        dc += 1;
        p += 2;
        c.output4pix(
            dc,
            [
                dc - 3,
                dc - 2 - x,
                dc - 1 - x,
                dc - x,
                dc - 3 + 2 * x,
                dc - 2 + x,
                dc - 1 + x,
                dc + x,
            ],
            [p - 1 - 2 * x, p - 2 * x, p - 1, p],
        );

        for _ in 3..x {
            dc += 1;
            p += 2;
            c.output4pix(
                dc,
                [
                    dc - 3 - x,
                    dc - 2 - x,
                    dc - 1 - x,
                    dc - x,
                    dc - 3 + x,
                    dc - 2 + x,
                    dc - 1 + x,
                    dc + x,
                ],
                [p - 1 - 2 * x, p - 2 * x, p - 1, p],
            );
        }
    }
}

/// ODD water: source = EVEN lines, destin = ODD lines.
fn odd(c: &mut WaterCells<'_>, x: i64, yd: i64, t: i64) {
    // LAST line first.
    let mut dc: i64 = (yd - 1) * 2 * x + x;
    let mut p: i64 = (yd - 1) * 4 * x + 2 * x + 1; // always lower right pixel dest

    c.output4pix(
        dc,
        [
            dc - 2,
            dc - 1,
            dc - x,
            dc + 1 - x,
            dc - t - 2 + 2 * x,
            dc - t - 1 + 2 * x,
            dc - t + x,
            dc - t + 1 + x,
        ],
        [p - 1 - 2 * x, p - 2 * x, p - 1, p],
    );

    dc += 1;
    p += 2;
    c.output4pix(
        dc,
        [
            dc - 2,
            dc - 1 - x,
            dc - x,
            dc + 1 - x,
            dc - t - 2 + 2 * x,
            dc - t - 1 + x,
            dc - t + x,
            dc - t + 1 + x,
        ],
        [p - 1 - 2 * x, p - 2 * x, p - 1, p],
    );

    // X=2 .. Xdimension-2 inclusive.
    for _ in 2..(x - 1) {
        dc += 1;
        p += 2;
        c.output4pix(
            dc,
            [
                dc - 2 - x,
                dc - 1 - x,
                dc - x,
                dc + 1 - x,
                dc - t - 2 + x,
                dc - t - 1 + x,
                dc - t + x,
                dc - t + 1 + x,
            ],
            [p - 1 - 2 * x, p - 2 * x, p - 1, p],
        );
    }

    // Last one needs SOURCE wrap to right.
    dc += 1;
    p += 2;
    c.output4pix(
        dc,
        [
            dc - 2 - x,
            dc - 1 - x,
            dc - x,
            dc + 1 - 2 * x,
            dc - t - 2 + x,
            dc - t - 1 + x,
            dc - t + x,
            dc - t + 1,
        ],
        [p - 1 - 2 * x, p - 2 * x, p - 1, p],
    );

    for y in 0..(yd - 1) {
        let mut dc = y * 2 * x + x;
        let mut p = y * 4 * x + 2 * x + 1;

        c.output4pix(
            dc,
            [
                dc - 2,
                dc - 1,
                dc - x,
                dc + 1 - x,
                dc - 2 + 2 * x,
                dc - 1 + 2 * x,
                dc + x,
                dc + 1 + x,
            ],
            [p - 1 - 2 * x, p - 2 * x, p - 1, p],
        );

        dc += 1;
        p += 2;
        c.output4pix(
            dc,
            [
                dc - 2,
                dc - 1 - x,
                dc - x,
                dc + 1 - x,
                dc - 2 + 2 * x,
                dc - 1 + x,
                dc + x,
                dc + 1 + x,
            ],
            [p - 1 - 2 * x, p - 2 * x, p - 1, p],
        );

        for _ in 2..(x - 1) {
            dc += 1;
            p += 2;
            c.output4pix(
                dc,
                [
                    dc - 2 - x,
                    dc - 1 - x,
                    dc - x,
                    dc + 1 - x,
                    dc - 2 + x,
                    dc - 1 + x,
                    dc + x,
                    dc + 1 + x,
                ],
                [p - 1 - 2 * x, p - 2 * x, p - 1, p],
            );
        }

        // Last one needs SOURCE wrap to right.
        dc += 1;
        p += 2;
        c.output4pix(
            dc,
            [
                dc - 2 - x,
                dc - 1 - x,
                dc - x,
                dc + 1 - 2 * x,
                dc - 2 + x,
                dc - 1 + x,
                dc + x,
                dc + 1,
            ],
            [p - 1 - 2 * x, p - 2 * x, p - 1, p],
        );
    }
}

/// `UWaterTexture::CalculateWater` (C path). `render` is `RenderTable` (1028 bytes), `wave`
/// is `WaveTable` (1536 bytes), `bitmap` is `Mips(0).DataArray`, `water` is the source fields.
pub fn calculate_water(
    trace: FireTraceHooks,
    st: &mut FireWaterState,
    bitmap: &mut [u8],
    water: &mut [u8],
    render: &[u8],
    wave: &[u8],
) {
    let mut g = globals::lock();
    g.trace = trace;

    let x = i64::from(st.u_size / 2); // Xdimension
    let yd = i64::from(st.v_size / 2); // Ydimension
    st.wave_parity = st.wave_parity.wrapping_add(1); // odd/even counter (BYTE)
    let t = 2 * x * yd; // TotalSize

    let mut cells = WaterCells {
        water,
        bitmap,
        render,
        wave,
    };
    if st.wave_parity & 1 == 0 {
        even(&mut cells, x, yd, t);
    } else {
        odd(&mut cells, x, yd, t);
    }
}

// ---------------------------------------------------------------------------
// WaterRedrawDrops.
// ---------------------------------------------------------------------------

/// Stores `v` into the two wave fields at `idx` (A then B) with the same value.
#[inline]
fn put_same(water: &mut [u8], m: Masks, idx: usize, v: u8) {
    put_ab(water, m.xsize, idx, v, v);
}

fn h_line(water: &mut [u8], m: Masks, xnow: i32, ynow: i32, seg: i32, depth: u8) {
    let base = ynow.wrapping_shl(m.ubits);
    for t in 0..=seg {
        let idx = base + ((t + xnow) & m.u2);
        put_same(water, m, idx as usize, depth);
    }
}

fn v_line(water: &mut [u8], m: Masks, xnow: i32, ynow: i32, seg: i32, depth: u8) {
    for t in 0..=seg {
        let add_y = ((ynow + t) & m.v2).wrapping_shl(m.ubits);
        let idx = xnow + add_y;
        put_same(water, m, idx as usize, depth);
    }
}

fn d1_line(water: &mut [u8], m: Masks, xnow: i32, ynow: i32, seg: i32, depth: u8) {
    for t in 0..=seg {
        let add_y = ((ynow + t) & m.v2).wrapping_shl(m.ubits);
        let add_x = (xnow - t) & m.u2;
        put_same(water, m, (add_y + add_x) as usize, depth);
    }
}

fn d2_line(water: &mut [u8], m: Masks, xnow: i32, ynow: i32, seg: i32, depth: u8) {
    for t in 0..=seg {
        let add_y = ((ynow + t) & m.v2).wrapping_shl(m.ubits);
        let add_x = (xnow + t) & m.u2;
        put_same(water, m, (add_y + add_x) as usize, depth);
    }
}

/// Whirly phase step shared by WhirlyThing / WhirlyBack (`shift` 4) and BigWhirly /
/// BigWhirlyBack (`shift` 3). `phase16` is the already-updated 16-bit phase.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
fn whirl_step(
    g: &FireGlobals,
    water: &mut [u8],
    m: Masks,
    xnow: i32,
    ynow: i32,
    d: &mut FireDrop,
    phase16: u32,
    shift: i32,
) {
    d.byte_a = (phase16 & 0xFF) as u8;
    d.byte_b = ((phase16 >> 8) & 0xFF) as u8;

    // So real 16-bit phase is B:A.
    let add_x = m.u2 & (xnow + (i32::from(g.phase_table[usize::from(d.byte_b)]) >> shift));
    let add_y = (m.v2
        & (ynow + (i32::from(g.phase_table[usize::from(d.byte_b.wrapping_add(64))]) >> shift)))
        .wrapping_shl(m.ubits);
    let idx = (add_x + add_y) as usize;
    put_same(water, m, idx, d.depth);
}

/// Edit-time `WaterRedrawDrops` body. Advances `GlobalPhase` and applies every drop.
fn redraw_drops(
    g: &mut FireGlobals,
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    water: &mut [u8],
) {
    let m = Masks {
        xsize: (st.u_size / 2) as usize,
        u2: i32::from((st.u_mask >> 1) as u8),
        v2: i32::from((st.v_mask >> 1) as u8),
        ubits: u32::from(st.u_bits),
    };

    st.global_phase = st.global_phase.wrapping_add(1);

    let mut s: i32 = 0;
    while s < st.num_drops {
        let si = s as usize;
        // Xnow/Ynow are copies taken before the switch; RandomMover updates X/Y only afterward.
        let xnow = i32::from(drops[si].x);
        let ynow = i32::from(drops[si].y);
        let dd = ynow.wrapping_shl(m.ubits).wrapping_add(xnow) as usize;
        let d = &mut drops[si];

        match d.ty {
            DROP_FIXED_DEPTH => {
                let depth = d.byte_d;
                put_same(water, m, dd, depth);
            }

            DROP_PHASE_SPOT => {
                d.depth = d.depth.wrapping_add(d.byte_d); // update phase
                let depth = g.phase_table[usize::from(d.depth)];
                put_same(water, m, dd, depth);
            }

            DROP_HALF_AMPL => {
                d.depth = d.depth.wrapping_add(d.byte_d); // update phase
                let mut depth = g.phase_table[usize::from(d.depth)];
                if depth < 128 {
                    depth = 128; // clamp it
                }
                put_same(water, m, dd, depth);
            }

            DROP_SHALLOW_SPOT => {
                d.depth = d.depth.wrapping_add(d.byte_d); // update phase
                // 64 + (PhaseTable >> 1) is at most 191, so it cannot wrap a BYTE.
                let depth = 64 + (g.phase_table[usize::from(d.depth)] >> 1);
                put_same(water, m, dd, depth);
            }

            DROP_RANDOM_MOVER => {
                // C leaves the order of the two SpeedRand() calls in each expression unspecified.
                // Port: left to right, X before Y.
                let r1 = i32::from(g.speed_rand() & 3);
                let r2 = i32::from(g.speed_rand() & 3);
                d.x = (m.u2 & (xnow - r1 + r2)) as u8;
                let r3 = i32::from(g.speed_rand() & 3);
                let r4 = i32::from(g.speed_rand() & 3);
                d.y = (m.v2 & (ynow - r3 + r4)) as u8;
                put_ab(water, m.xsize, dd, 128 + 57, 128 - 57);
            }

            DROP_FIXED_RANDOM_SPOT => {
                let a = g.speed_rand();
                water[dd] = a;
                let b = g.speed_rand();
                water[m.xsize + dd] = b;
            }

            DROP_WHIRLY_THING => {
                // 16-bit phase: B:A += D:C.
                let phase16 = ((u32::from(d.byte_b) << 8) + u32::from(d.byte_a))
                    .wrapping_add((u32::from(d.byte_d) << 8) + u32::from(d.byte_c))
                    & 0xFFFF;
                whirl_step(g, water, m, xnow, ynow, d, phase16, 4);
            }

            DROP_WHIRLY_BACK => {
                // 16-bit phase: B:A -= D:C.
                let phase16 = ((u32::from(d.byte_b) << 8) + u32::from(d.byte_a))
                    .wrapping_sub(u32::from(d.byte_d) << 8)
                    .wrapping_sub(u32::from(d.byte_c))
                    & 0xFFFF;
                whirl_step(g, water, m, xnow, ynow, d, phase16, 4);
            }

            DROP_BIG_WHIRLY => {
                // C reads `*(DWORD*)&ByteA` and `*(_WORD*)&ByteC` (little-endian punning).
                let p32 = u32::from_le_bytes([d.byte_a, d.byte_b, d.byte_c, d.byte_d]);
                let cd16 = u32::from(u16::from_le_bytes([d.byte_c, d.byte_d]));
                let phase16 = p32.wrapping_add(cd16) & 0xFFFF;
                whirl_step(g, water, m, xnow, ynow, d, phase16, 3);
            }

            DROP_BIG_WHIRLY_BACK => {
                let phase16 = ((u32::from(d.byte_b) << 8) + u32::from(d.byte_a))
                    .wrapping_sub(u32::from(d.byte_d) << 8)
                    .wrapping_sub(u32::from(d.byte_c))
                    & 0xFFFF;
                whirl_step(g, water, m, xnow, ynow, d, phase16, 3);
            }

            DROP_HORIZONTAL_LINE => {
                let depth = d.depth;
                let seg = i32::from(d.byte_d >> 1);
                h_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_VERTICAL_LINE => {
                let depth = d.depth;
                let seg = i32::from(d.byte_d >> 1);
                v_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_DIAGONAL_LINE1 => {
                let depth = d.depth;
                let seg = i32::from(d.byte_d >> 1);
                d1_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_DIAGONAL_LINE2 => {
                let depth = d.depth;
                let seg = i32::from(d.byte_d >> 1);
                d2_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_HORIZONTAL_OSC => {
                d.depth = d.depth.wrapping_add(d.byte_c); // update phase
                let depth = g.phase_table[usize::from(d.depth)];
                let seg = i32::from(d.byte_d >> 1);
                h_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_VERTICAL_OSC => {
                d.depth = d.depth.wrapping_add(d.byte_c); // update phase
                let depth = g.phase_table[usize::from(d.depth)];
                let seg = i32::from(d.byte_d >> 1);
                v_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_DIAGONAL_OSC1 => {
                d.depth = d.depth.wrapping_add(d.byte_c); // update phase
                let depth = g.phase_table[usize::from(d.depth)];
                let seg = i32::from(d.byte_d >> 1);
                d1_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_DIAGONAL_OSC2 => {
                d.depth = d.depth.wrapping_add(d.byte_c); // update phase
                let depth = g.phase_table[usize::from(d.depth)];
                let seg = i32::from(d.byte_d >> 1);
                d2_line(water, m, xnow, ynow, seg, depth);
            }

            DROP_AREA_CLAMP => {
                // The C initial `DestY` assignment is dead (overwritten in the v loop) and is omitted.
                let depth = d.depth;
                let seg = i32::from(d.byte_d >> 1);
                for v in 0..seg {
                    let dest_y = ((ynow + v) & m.v2).wrapping_shl(m.ubits);
                    let mut dest = dest_y + (xnow & m.u2);
                    for u in 1..=seg {
                        put_same(water, m, dest as usize, depth);
                        dest = dest_y + ((u + xnow) & m.u2);
                    }
                }
            }

            DROP_RAIN_DROPS => {
                // Keep the action sparse.
                if (g.speed_rand() & 15) == 0 {
                    let depth = d.depth;
                    let spray = i32::from(d.byte_d);
                    let r1 = i32::from(g.speed_rand());
                    let add_y = ((ynow + ((r1 * spray) >> 8)) & m.v2).wrapping_shl(m.ubits);
                    let r2 = i32::from(g.speed_rand());
                    let add_x = (xnow + ((r2 * spray) >> 8)) & m.u2;
                    put_ab(water, m.xsize, (add_y + add_x) as usize, depth, 255 ^ depth);
                }
            }

            DROP_LEAKY_TAP => {
                // `(ByteA += ByteD) <= ByteD`: compares the updated BYTE.
                d.byte_a = d.byte_a.wrapping_add(d.byte_d);
                if i32::from(d.byte_a) <= i32::from(d.byte_d) {
                    let depth = d.depth;
                    put_ab(water, m.xsize, dd, depth, 255 ^ depth);
                }
            }

            DROP_DRIPPY_TAP => {
                d.byte_a = d.byte_a.wrapping_add(d.byte_d);
                if i32::from(d.byte_a) <= i32::from(d.byte_d) {
                    d.byte_a = g.speed_rand();
                    let depth = d.depth;
                    put_ab(water, m.xsize, dd, depth, 255 ^ depth);
                }
            }

            _ => {}
        }

        s += 1;
    }
}

/// `UWaterTexture::WaterRedrawDrops`.
pub fn water_redraw_drops(
    trace: FireTraceHooks,
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    water: &mut [u8],
) {
    let mut g = globals::lock();
    g.trace = trace;
    redraw_drops(&mut g, st, drops, water);
}

// ---------------------------------------------------------------------------
// AddDrop / DeleteDrops.
// ---------------------------------------------------------------------------

/// Type-specific setup of a whirly drop. `back_ty` is the negative-variant type.
fn add_whirl(d: &mut FireDrop, gp: i32, fx_freq: u8, fx_phase: u8, fx_depth: u8, back_ty: u8) {
    d.depth = fx_depth;
    d.byte_a = 0; // Low-byte of phase.

    let mut temp_freq = i32::from(fx_freq);
    if temp_freq > 127 {
        // negative variant
        d.ty = back_ty;
        temp_freq = 255 - temp_freq;
    }

    // Divisor (127-TempFreq)+1 is in 1..=128, never zero.
    let time_delta: u32 = 65535 / ((127 - temp_freq) + 1) as u32;
    d.byte_c = (time_delta & 0x00FF) as u8;
    d.byte_d = ((time_delta & 0xFF00) >> 8) as u8;
    // GlobalPhase * TimeDelta is DWORD multiplication; (INT) GlobalPhase converts to DWORD.
    let v = (gp as u32)
        .wrapping_mul(time_delta)
        .wrapping_add(u32::from(fx_phase) << 8);
    d.byte_b = ((v >> 8) & 255) as u8;
}

/// `UWaterTexture::AddDrop` (edit-time). Caller holds the globals lock.
fn add_drop_locked(
    g: &mut FireGlobals,
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    drop_x: i32,
    drop_y: i32,
) {
    // Return if out of bounds or out of Drops.
    if drop_x >= st.u_size
        || drop_y >= st.v_size
        || drop_x < 0
        || drop_y < 0
        || st.num_drops >= MAX_DROPS
    {
        return;
    }

    let s = st.num_drops as usize;
    st.num_drops += 1;

    let gp = st.global_phase;
    let fx_freq = st.fx_frequency;
    let fx_phase = st.fx_phase;
    let fx_size = st.fx_size;
    let fx_depth = st.fx_depth;
    let drop_type = st.drop_type;

    let d = &mut drops[s];
    d.x = (drop_x >> 1) as u8;
    d.y = (drop_y >> 1) as u8;
    d.ty = drop_type;

    st.draw_phase = st.draw_phase.wrapping_add(1);

    match drop_type {
        DROP_PHASE_SPOT | DROP_HALF_AMPL => {
            // Align new phase in-step.
            d.depth = (gp
                .wrapping_mul(i32::from(fx_freq))
                .wrapping_add(i32::from(fx_phase))
                & 255) as u8;
            d.byte_d = fx_freq;
            d.byte_a = 0;
            d.byte_b = 0;
            d.byte_c = 0;
        }

        DROP_SHALLOW_SPOT => {
            d.depth = (gp
                .wrapping_mul(i32::from(fx_freq))
                .wrapping_add(i32::from(fx_phase))
                & 255) as u8;
            d.byte_d = fx_freq;
        }

        DROP_FIXED_RANDOM_SPOT => {}

        DROP_HORIZONTAL_LINE | DROP_VERTICAL_LINE | DROP_DIAGONAL_LINE1 | DROP_DIAGONAL_LINE2
        | DROP_RAIN_DROPS => {
            d.depth = fx_depth;
            d.byte_d = fx_size;
        }

        DROP_HORIZONTAL_OSC | DROP_VERTICAL_OSC | DROP_DIAGONAL_OSC1 | DROP_DIAGONAL_OSC2 => {
            d.depth = (gp
                .wrapping_mul(i32::from(fx_freq))
                .wrapping_add(i32::from(fx_phase))
                & 255) as u8;
            d.byte_c = fx_freq;
            d.byte_d = fx_size;
        }

        DROP_AREA_CLAMP => {
            d.depth = fx_depth;
            d.byte_d = fx_size;
        }

        DROP_WHIRLY_THING => {
            add_whirl(d, gp, fx_freq, fx_phase, fx_depth, DROP_WHIRLY_BACK);
        }

        DROP_BIG_WHIRLY => {
            add_whirl(d, gp, fx_freq, fx_phase, fx_depth, DROP_BIG_WHIRLY_BACK);
        }

        DROP_LEAKY_TAP => {
            d.depth = fx_depth;
            d.byte_a = (gp
                .wrapping_mul(i32::from(fx_freq))
                .wrapping_add(i32::from(fx_phase))
                & 255) as u8;
            d.byte_d = fx_freq >> 2;
        }

        DROP_DRIPPY_TAP => {
            d.depth = fx_depth;
            d.byte_a = g.speed_rand();
            d.byte_d = fx_freq >> 2;
        }

        _ => {}
    }
}

/// `UWaterTexture::AddDrop`.
pub fn add_drop(
    trace: FireTraceHooks,
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    drop_x: i32,
    drop_y: i32,
) {
    let mut g = globals::lock();
    g.trace = trace;
    add_drop_locked(&mut g, st, drops, drop_x, drop_y);
}

/// `UWaterTexture::DeleteDrops` body. Caller holds the globals lock (not RNG-dependent).
fn delete_drops_locked(
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    drop_x: i32,
    drop_y: i32,
    area_width: i32,
) {
    let mut s: i32 = 0;
    while s < st.num_drops {
        let si = s as usize;
        let dx = drop_x - (i32::from(drops[si].x) << 1);
        let dy = drop_y - (i32::from(drops[si].y) << 1);
        if area_width >= dx.abs() + dy.abs() {
            // Delete Drop by replacing it with last one (+ delete last one).
            st.num_drops -= 1;
            let last = st.num_drops as usize;
            drops[si] = drops[last];
        }
        s += 1;
    }
}

/// `UWaterTexture::DeleteDrops`.
pub fn delete_drops(
    trace: FireTraceHooks,
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    drop_x: i32,
    drop_y: i32,
    area_width: i32,
) {
    let mut g = globals::lock();
    g.trace = trace;
    delete_drops_locked(st, drops, drop_x, drop_y, area_width);
}

// ---------------------------------------------------------------------------
// WaterPaint / TouchTexture / TempDrawSpark.
// ---------------------------------------------------------------------------

/// `UWaterTexture::WaterPaint`.
pub fn water_paint(
    trace: FireTraceHooks,
    st: &mut FireWaterState,
    drops: &mut [FireDrop],
    x: i32,
    y: i32,
    buttons: u32,
) {
    let mut g = globals::lock();
    g.trace = trace;

    let mouse_x = x;
    let mouse_y = y;
    let left_button = (buttons & MOUSE_LEFT) != 0;

    // C `static INT LastMouseX/Y, LastLeftButton, LastRightButton` in this function are never
    // assigned anywhere, so they stay 0 and the comparisons reduce to the values below.
    let pos_changed = mouse_x != 0 || mouse_y != 0;
    let left_changed = left_button;

    // Types that don't require continuous drawing.
    let continuous = !matches!(
        st.drop_type,
        DROP_HORIZONTAL_LINE..=DROP_DIAGONAL_OSC2 | DROP_AREA_CLAMP
    );

    // Paint on the water.
    if (buttons & MOUSE_LEFT) != 0 && continuous && (pos_changed || left_changed) {
        add_drop_locked(&mut g, st, drops, mouse_x, mouse_y);
    }

    if (buttons & MOUSE_RIGHT) != 0 {
        delete_drops_locked(st, drops, mouse_x, mouse_y, DEL_RANGE);
    }
}

/// `UWaterTexture::TouchTexture`.
pub fn touch_texture(
    trace: FireTraceHooks,
    st: &FireWaterState,
    water: &mut [u8],
    u_pos: i32,
    v_pos: i32,
    magnitude: f32,
) {
    let mut g = globals::lock();
    g.trace = trace;

    let xsize = st.u_size as usize; // WaveFieldB = WaveFieldA + USize (not USize/2)
    let drop_dest = (v_pos.wrapping_shl(u32::from(st.u_bits)).wrapping_add(u_pos)) as u32 as usize;

    // Depress a circular gradual area, radius = Magnitude. (BYTE) float conversion truncates.
    let depth = (magnitude as i32) as u8;
    // Divergence: C writes WaveFieldA[DropDest] and WaveFieldB[DropDest] unchecked. Skip both
    // writes when either index is out of range, so there is no partial write and no panic.
    if drop_dest >= water.len() || xsize + drop_dest >= water.len() {
        return;
    }
    put_ab(water, xsize, drop_dest, depth, depth);
}

/// `UFireTexture::TempDrawSpark` (editor). Writes `Mips(0).DataArray` directly.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
pub fn temp_draw_spark(
    trace: FireTraceHooks,
    bitmap: &mut [u8],
    u_mask: i32,
    v_mask: i32,
    u_bits: u8,
    pos_x: i32,
    pos_y: i32,
    intensity: i32,
) {
    let mut g = globals::lock();
    g.trace = trace;

    let spark_dest = ((u_mask & pos_x)
        .wrapping_add((v_mask & pos_y).wrapping_shl(u32::from(u_bits)))) as u32
        as usize;
    bitmap[spark_dest] = intensity as u8;
}
