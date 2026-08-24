//! DXT1 (BC1) decode — runtime path only; packages carry pre-encoded
//! blocks. Semantics pinned by `Tests/Dxt1Tests.cpp`: standard BC1 with
//! three-color mode selected by numeric `c0 <= c1`, `color2 = lerp(c0,c1,⅓)`
//! in four-color mode, `color2 = average` in three-color mode, index 3 =
//! transparent there. Output is BGRA word order (the engine's ARGB8888
//! destination layout).

/// Decoder rejection reasons (`dxt.*` reason-code prefixes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DxtError {
    /// Block payload does not cover the rounded-up 4x4 block grid.
    Truncated { need: usize, got: usize },
    /// Zero-sized surfaces are rejected loudly.
    EmptyDimensions,
}

impl std::fmt::Display for DxtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DxtError::Truncated { need, got } => {
                write!(f, "dxt.truncated: need {need} block bytes, got {got}")
            }
            DxtError::EmptyDimensions => write!(f, "dxt.empty_dimensions"),
        }
    }
}

impl std::error::Error for DxtError {}

pub const BLOCK_BYTES: usize = 8;
const BLOCK_EDGE: usize = 4;

/// Wire size of an encoded DXT1 surface (whole 4x4 blocks).
#[must_use]
pub fn encoded_size(width: u32, height: u32) -> usize {
    let blocks_x = width.div_ceil(BLOCK_EDGE as u32) as usize;
    let blocks_y = height.div_ceil(BLOCK_EDGE as u32) as usize;
    blocks_x * blocks_y * BLOCK_BYTES
}

/// Byte length of a decoded BGRA surface.
#[must_use]
pub fn decoded_size(width: u32, height: u32) -> usize {
    width as usize * height as usize * 4
}

/// Decode a DXT1 block stream into tightly packed BGRA8 rows.
///
/// Dimensions may be non-multiples of four; edge texels come from the same
/// block grid exactly as the engine crops them.
pub fn decode_dxt1_to_bgra(blocks: &[u8], width: u32, height: u32) -> Result<Vec<u8>, DxtError> {
    if width == 0 || height == 0 {
        return Err(DxtError::EmptyDimensions);
    }
    let blocks_x = width.div_ceil(BLOCK_EDGE as u32) as usize;
    let blocks_y = height.div_ceil(BLOCK_EDGE as u32) as usize;
    let need = blocks_x * blocks_y * BLOCK_BYTES;
    if blocks.len() < need {
        return Err(DxtError::Truncated {
            need,
            got: blocks.len(),
        });
    }

    let mut out = vec![0u8; decoded_size(width, height)];
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let base = (block_y * blocks_x + block_x) * BLOCK_BYTES;
            let (palette, indices) = unpack_block(&blocks[base..base + BLOCK_BYTES]);
            for texel in 0..16usize {
                let tx = block_x * BLOCK_EDGE + texel % BLOCK_EDGE;
                let ty = block_y * BLOCK_EDGE + texel / BLOCK_EDGE;
                if tx >= width as usize || ty >= height as usize {
                    continue;
                }
                let color = palette[((indices >> (texel * 2)) & 3) as usize];
                let offset = (ty * width as usize + tx) * 4;
                out[offset] = color[2]; // B
                out[offset + 1] = color[1]; // G
                out[offset + 2] = color[0]; // R
                out[offset + 3] = color[3]; // A
            }
        }
    }
    Ok(out)
}

type Palette = [[u8; 4]; 4];

/// Expand one block into its BGRA palette plus the packed 2-bit indices.
fn unpack_block(block: &[u8]) -> (Palette, u32) {
    let color0 = u16::from_le_bytes([block[0], block[1]]);
    let color1 = u16::from_le_bytes([block[2], block[3]]);
    let c0 = rgb565_to_bgra8(color0);
    let c1 = rgb565_to_bgra8(color1);
    let indices = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    let mut palette = [[0u8; 4]; 4];
    palette[0] = c0;
    palette[1] = c1;
    if color0 > color1 {
        // Four-color opaque mode.
        for channel in 0..3 {
            palette[2][channel] = ((2 * i32::from(c0[channel]) + i32::from(c1[channel])) / 3) as u8;
            palette[3][channel] = ((i32::from(c0[channel]) + 2 * i32::from(c1[channel])) / 3) as u8;
        }
        palette[2][3] = 255;
        palette[3][3] = 255;
    } else {
        // Three-color + transparent mode: index 3 is fully transparent,
        // index 2 is the plain average of the endpoint colors.
        for channel in 0..3 {
            palette[2][channel] = ((i32::from(c0[channel]) + i32::from(c1[channel])) / 2) as u8;
        }
        palette[2][3] = 255;
        palette[3] = [0, 0, 0, 0];
    }
    (palette, indices)
}

/// RGB565 → `{R, G, B, A}` with the standard bit-replication expansion
/// (5→8 bits replicates top bits, 6→8 shifts in the top two bits), which
/// reproduces the pinned 85/170 blends exactly.
fn rgb565_to_bgra8(pixel: u16) -> [u8; 4] {
    let r5 = ((pixel >> 11) & 0x1f) as u8;
    let g6 = ((pixel >> 5) & 0x3f) as u8;
    let b5 = (pixel & 0x1f) as u8;
    [
        (r5 << 3) | (r5 >> 2),
        (g6 << 2) | (g6 >> 4),
        (b5 << 3) | (b5 >> 2),
        255,
    ]
}
