//! Ports of the decode-side cases in Tests/Dxt1Tests.cpp (dxt1_codec).

use hp_format::dxt1::{decode_dxt1_to_bgra, decoded_size, encoded_size, DxtError};

#[test]
fn known_four_colour_block_channel_and_index_order() {
    // c0=red, c1=green, every index byte 0xE4 -> row of indices 0,1,2,3.
    let block = [0x00, 0xf8, 0xe0, 0x07, 0xe4, 0xe4, 0xe4, 0xe4];
    let pixels = decode_dxt1_to_bgra(&block, 4, 4).expect("decode");
    assert_eq!(pixels.len(), decoded_size(4, 4));
    // BGRA rows; identical for all four rows.
    let row = |y: usize| &pixels[y * 16..(y + 1) * 16];
    for y in 0..4 {
        assert_eq!(&row(y)[0..4], &[0, 0, 255, 255], "idx0 red at (0, {y})");
        assert_eq!(&row(y)[4..8], &[0, 255, 0, 255], "idx1 green");
        assert_eq!(&row(y)[8..12], &[0, 85, 170, 255], "idx2 = 2*c0/3+c1/3");
        assert_eq!(&row(y)[12..16], &[0, 170, 85, 255], "idx3 = c0/3+2*c1/3");
    }
}

#[test]
fn known_transparent_block_three_colour_mode() {
    // c0=black <= c1=white numerically: three-colour mode with average
    // midpoint and transparent index 3.
    let block = [0x00, 0x00, 0xff, 0xff, 0xe4, 0xe4, 0xe4, 0xe4];
    let pixels = decode_dxt1_to_bgra(&block, 4, 4).expect("decode");
    assert_eq!(&pixels[0..4], &[0, 0, 0, 255], "idx0 black");
    assert_eq!(&pixels[4..8], &[255, 255, 255, 255], "idx1 white");
    assert_eq!(&pixels[8..12], &[127, 127, 127, 255], "idx2 average");
    assert_eq!(&pixels[12..16], &[0, 0, 0, 0], "idx3 transparent");
}

#[test]
fn block_cropping_and_sizes() {
    // Two blocks decoding a 5x3 surface: sizes round up to whole blocks,
    // decoded size is exactly width*height*4, edge texels crop cleanly.
    let blocks = [
        0x00, 0xf8, 0x00, 0x00, 0, 0, 0, 0, // red endpoints, zero indices
        0xe0, 0x07, 0x00, 0x00, 0, 0, 0, 0, // green endpoints, zero indices
    ];
    assert_eq!(encoded_size(5, 3), 16, "encode size rounds to full blocks");
    assert_eq!(decoded_size(5, 3), 5 * 3 * 4);

    let pixels = decode_dxt1_to_bgra(&blocks, 5, 3).expect("crop decode");
    // First texel of each row comes from the left (red) block.
    for y in 0..3usize {
        let offset = (y * 5) * 4;
        assert_eq!(&pixels[offset..offset + 4], &[0, 0, 255, 255]);
    }
}

#[test]
fn truncated_block_stream_is_loud() {
    let short = [0u8; 7]; // one byte short of a full block
    assert_eq!(
        decode_dxt1_to_bgra(&short, 4, 4),
        Err(DxtError::Truncated { need: 8, got: 7 })
    );
    assert_eq!(
        decode_dxt1_to_bgra(&[], 0, 0),
        Err(DxtError::EmptyDimensions)
    );
}
