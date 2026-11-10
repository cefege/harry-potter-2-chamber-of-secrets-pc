//! CPU conversion of legacy texture pixels to RGBA8.

use hp2_abi::FColor;

/// Controls whether palette index zero is transparent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P8MaskMode {
    /// Preserve every palette entry.
    Unmasked,
    /// Replace palette entry zero with transparent black.
    Masked,
}

/// Expands indexed texels using the 256-entry RGBA palette.
///
/// Masked textures use transparent black for index zero without changing the input palette.
pub fn p8_to_rgba8(indices: &[u8], palette: &[FColor; 256], mask_mode: P8MaskMode) -> Vec<FColor> {
    let mut local_palette;
    let palette = match mask_mode {
        P8MaskMode::Unmasked => palette,
        P8MaskMode::Masked => {
            local_palette = *palette;
            local_palette[0] = FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
            &local_palette
        }
    };
    indices
        .iter()
        .map(|&index| palette[index as usize])
        .collect()
}

/// Expands indexed texels into owned RGBA bytes without modifying the palette.
///
/// Masked textures use transparent black for palette index zero.
pub fn p8_to_rgba8_bytes(
    indices: &[u8],
    palette: &[FColor; 256],
    mask_mode: P8MaskMode,
) -> Vec<u8> {
    let mut local_palette;
    let palette = match mask_mode {
        P8MaskMode::Unmasked => palette,
        P8MaskMode::Masked => {
            local_palette = *palette;
            local_palette[0] = FColor {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
            &local_palette
        }
    };
    indices
        .iter()
        .flat_map(|&index| {
            let color = palette[index as usize];
            [color.r, color.g, color.b, color.a]
        })
        .collect()
}

/// Expands tightly packed RGB8 texels and sets alpha to 255.
///
/// # Panics
/// Panics if the input length is not a multiple of 3.
pub fn rgb8_to_rgba8(rgb_bytes: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgb_bytes.len() % 3,
        0,
        "rgb_bytes length must be a whole multiple of 3"
    );
    rgb_bytes
        .chunks_exact(3)
        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
        .collect()
}

/// Expands little-endian RGB565 texels and sets alpha to 255.
///
/// The `GL_UNSIGNED_SHORT_5_6_5_REV` layout places red in the low five bits,
/// green in the next six bits, and blue in the high five bits. Bit replication
/// expands each channel to eight bits.
///
/// # Panics
/// Panics if the input length is not a multiple of 2.
pub fn rgb16_to_rgba8(packed_bytes: &[u8]) -> Vec<u8> {
    assert_eq!(
        packed_bytes.len() % 2,
        0,
        "packed_bytes length must be a whole multiple of 2"
    );
    packed_bytes
        .chunks_exact(2)
        .flat_map(|pair| {
            let value = u16::from_le_bytes([pair[0], pair[1]]);
            let r5 = (value & 0x1F) as u8;
            let g6 = ((value >> 5) & 0x3F) as u8;
            let b5 = ((value >> 11) & 0x1F) as u8;
            let r8 = (r5 << 3) | (r5 >> 2);
            let g8 = (g6 << 2) | (g6 >> 4);
            let b8 = (b5 << 3) | (b5 >> 2);
            [r8, g8, b8, 255]
        })
        .collect()
}
