//! DXT1/BC1 block decode, replacing the single call to `squish::Decompress`
//! in `HarryPotter2/Unreal/Engine/Src/S3tcCompat.cpp`'s `S3TCdecode`. Encode
//! (`squish::CompressMasked`, called from `S3TCencode`) is out of scope —
//! the plan only replaces decode, and `S3TCdecode`'s DDSURFACEDESC parsing,
//! pitch/bounds handling and output-descriptor construction stay in C++;
//! this is the pure per-block math only (8 bytes in, 64 bytes of RGBA out).
//!
//! Behavior verified against `Tests/Dxt1Tests.cpp`'s pinned byte vectors
//! (`TestKnownFourColourBlock`, `TestKnownTransparentBlock`): RGB565
//! expansion by bit replication, 4-color (`color0 > color1`) vs. 3-color
//! plus transparent (`color0 <= color1`) interpolation in 8-bit RGB space
//! with truncating integer division, and 2-bit indices read LSB-first with
//! one index byte per pixel row — the standard DXT1 layout.

fn expand565(value: u16) -> [u8; 3] {
    let r5 = ((value >> 11) & 0x1f) as u32;
    let g6 = ((value >> 5) & 0x3f) as u32;
    let b5 = (value & 0x1f) as u32;
    [
        ((r5 << 3) | (r5 >> 2)) as u8,
        ((g6 << 2) | (g6 >> 4)) as u8,
        ((b5 << 3) | (b5 >> 2)) as u8,
    ]
}

/// Decodes one 8-byte DXT1 block into 16 RGBA pixels (64 bytes, row-major,
/// 4 bytes per pixel), matching `squish::Decompress(.., squish::kDxt1)`.
pub fn decode_block(block: &[u8; 8]) -> [u8; 64] {
    let color0 = u16::from_le_bytes([block[0], block[1]]);
    let color1 = u16::from_le_bytes([block[2], block[3]]);
    let c0 = expand565(color0);
    let c1 = expand565(color1);
    let (c2, c3, index3_transparent) = if color0 > color1 {
        let mix = |a: u8, b: u8, wa: u32, wb: u32| ((a as u32 * wa + b as u32 * wb) / 3) as u8;
        (
            [
                mix(c0[0], c1[0], 2, 1),
                mix(c0[1], c1[1], 2, 1),
                mix(c0[2], c1[2], 2, 1),
            ],
            [
                mix(c0[0], c1[0], 1, 2),
                mix(c0[1], c1[1], 1, 2),
                mix(c0[2], c1[2], 1, 2),
            ],
            false,
        )
    } else {
        let avg = |a: u8, b: u8| (((a as u32) + (b as u32)) / 2) as u8;
        (
            [avg(c0[0], c1[0]), avg(c0[1], c1[1]), avg(c0[2], c1[2])],
            [0u8, 0, 0],
            true,
        )
    };
    let colors = [c0, c1, c2, c3];
    let alphas: [u8; 4] = [255, 255, 255, if index3_transparent { 0 } else { 255 }];
    let mut out = [0u8; 64];
    for row in 0..4usize {
        let byte = block[4 + row];
        for col in 0..4usize {
            let index = ((byte >> (col * 2)) & 0b11) as usize;
            let pixel = (row * 4 + col) * 4;
            out[pixel] = colors[index][0];
            out[pixel + 1] = colors[index][1];
            out[pixel + 2] = colors[index][2];
            out[pixel + 3] = alphas[index];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_four_colour_block_matches_pinned_vector() {
        // Tests/Dxt1Tests.cpp TestKnownFourColourBlock, RGBA order here
        // (S3tcCompat.cpp swaps R/B itself when writing BGRA).
        let block = [0x00, 0xf8, 0xe0, 0x07, 0xe4, 0xe4, 0xe4, 0xe4];
        let pixels = decode_block(&block);
        let expected_row = [
            [255u8, 0, 0, 255],
            [0, 255, 0, 255],
            [170, 85, 0, 255],
            [85, 170, 0, 255],
        ];
        for row in 0..4 {
            for (col, exp) in expected_row.iter().enumerate() {
                let p = (row * 4 + col) * 4;
                assert_eq!(&pixels[p..p + 4], &exp[..], "row {row} col {col}");
            }
        }
    }

    #[test]
    fn known_transparent_block_matches_pinned_vector() {
        // Tests/Dxt1Tests.cpp TestKnownTransparentBlock.
        let block = [0x00, 0x00, 0xff, 0xff, 0xe4, 0xe4, 0xe4, 0xe4];
        let pixels = decode_block(&block);
        let expected = [
            [0u8, 0, 0, 255],
            [255, 255, 255, 255],
            [127, 127, 127, 255],
            [0, 0, 0, 0],
        ];
        for (col, exp) in expected.iter().enumerate() {
            let p = col * 4;
            assert_eq!(&pixels[p..p + 4], &exp[..], "col {col}");
        }
    }
}
