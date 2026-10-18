//! Port of `BlueLagunaPalette`. The palette is
//! computed into a plain RGB buffer. The C++ shell copies the channels into
//! `UPalette::Colors`, so the engine's `FColor` stays in C++.

/// One palette entry. Mirrors the `R`, `G`, `B` bytes of `FColor`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Each quarter is `(base, step)` per channel. C: `Min(base + (i*step)/64, 255)`.
/// Quarter `q` covers entries `i+64*q` for `i` in `0..64`.
const QUARTERS: [[(i32, i32); 3]; 4] = [
    [(0, 59), (0, 67), (0, 100)],
    [(59, 55), (67, 60), (100, 97)],
    [(114, 64), (127, 60), (197, 33)],
    [(178, 78), (187, 69), (230, 26)],
];

/// Fills all 256 entries. C writes `Colors(i).R/G/B` for `i` in `0..256`.
pub fn blue_laguna_palette(out: &mut [FireRgb]) {
    assert!(out.len() >= 256, "blue_laguna_palette: need 256 entries");
    for i in 0..64i32 {
        for (q, chans) in QUARTERS.iter().enumerate() {
            let idx = (i + 64 * q as i32) as usize;
            let (rb, rs) = chans[0];
            let (gb, gs) = chans[1];
            let (bb, bs) = chans[2];
            // C: Min( base + (i*step)/64, 255 ). Integer division, so `/` truncates as in C.
            out[idx] = FireRgb {
                r: (rb + (i * rs) / 64).min(255) as u8,
                g: (gb + (i * gs) / 64).min(255) as u8,
                b: (bb + (i * bs) / 64).min(255) as u8,
            };
        }
    }
}
