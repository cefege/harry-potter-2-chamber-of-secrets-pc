//! Legacy MPEG header parsing and layer-II frame-size policy.
//!
//! This intentionally preserves the source parser's 12-bit sync mask. That
//! mask makes version 2.5 unreachable even though its table branch remains in
//! the original control flow. The parser writes fields in the original order;
//! rejected headers therefore retain the same partially-written native record.

use core::ffi::c_void;

const SAMPLE_RATE: [[u16; 4]; 2] = [[22_050, 24_000, 16_000, 0], [44_100, 48_000, 32_000, 0]];
const BIT_RATE: [[[u32; 15]; 3]; 2] = [
    [
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ],
    [
        [
            0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ],
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ],
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ],
    ],
];

/// Exact native `MPEGAUDIOHDR` layout. `pad_short` and `pad_char` are not
/// initialized by the parser; keeping them here prevents accidental clearing.
#[repr(C)]
struct MpegAudioHeader {
    bitrate: u32,
    samplerate: u16,
    numframes: u16,
    framebytes: u16,
    pad_short: u16,
    version: u8,
    layer: u8,
    channels: u8,
    mode: u8,
    modeext: u8,
    crc: u8,
    padded: u8,
    samplerateindex: u8,
    bitrateindex: u8,
    pad_char: [u8; 3],
}

#[inline]
fn has_sync(header: u32) -> bool {
    header & 0xfff0_0000 == 0xfff0_0000
}

/// Parse a packed MPEG header into the live native record. Invalid sync never
/// dereferences `output`, as the C++ implementation does. For a synchronized
/// invalid header, every write and early return is in original source order.
///
/// # Safety
/// On synchronized input `output` is a live, aligned `MPEGAUDIOHDR` pointer.
/// Its non-parser-owned padding is preserved exactly. Null/unaligned/invalid
/// storage is outside the original function's defined contract.
pub unsafe fn parse_header(header: u32, output: *mut c_void) -> i32 {
    if !has_sync(header) {
        return 0;
    }
    let output = unsafe { &mut *output.cast::<MpegAudioHeader>() };

    output.layer = 4 - ((header >> 17) & 3) as u8;
    output.crc = ((header >> 16) & 1 == 0) as u8;
    output.bitrateindex = ((header >> 12) & 15) as u8;
    output.padded = ((header >> 9) & 1) as u8;
    output.mode = ((header >> 6) & 3) as u8;
    output.modeext = ((header >> 4) & 3) as u8;

    if output.layer == 4 || output.bitrateindex == 15 {
        return 0;
    }

    output.version = ((header >> 19) & 3) as u8;
    if output.version == 1 {
        return 0;
    }

    output.samplerateindex = ((header >> 10) & 3) as u8;
    if output.samplerateindex == 3 {
        return 0;
    }

    output.channels = if output.mode == 3 { 1 } else { 2 };
    output.samplerate = SAMPLE_RATE[1][output.samplerateindex as usize];
    if output.version == 2 {
        output.samplerate >>= 1;
    }
    if output.version == 0 {
        output.samplerate >>= 2;
    }
    if output.bitrateindex == 0 {
        return 0;
    }

    output.bitrate = BIT_RATE[(output.version == 3) as usize][output.layer as usize - 1]
        [output.bitrateindex as usize];
    if output.layer == 1 {
        output.framebytes = ((12_000 * output.bitrate) / u32::from(output.samplerate)) as u16;
        output.framebytes = (output.framebytes + u16::from(output.padded)) << 2;
        output.numframes = 384;
    } else {
        output.numframes = 1152;
        output.framebytes = ((144_000 * output.bitrate) / u32::from(output.samplerate)) as u16;
        if output.layer == 3 && output.version != 3 {
            output.framebytes >>= 1;
            output.numframes = 576;
        }
        if output.padded != 0 {
            output.framebytes += 1;
        }
    }
    i32::from(output.framebytes)
}

/// Return the frame size for the original layer-II-only helper. `None` means
/// the C++ shell must execute its original `check(0)` fault path. A zero sample
/// rate reaches undefined division in the original helper and is intentionally
/// outside this defined-input function's contract.
pub fn frame_size(header: u32) -> Option<i32> {
    if !has_sync(header) || 4 - ((header >> 17) & 3) != 2 {
        return None;
    }
    let bitrate_index = ((header >> 12) & 15) as usize;
    let padded = (header >> 9) & 1;
    let version = (header >> 19) & 3;
    let mut sample_rate = SAMPLE_RATE[1][((header >> 10) & 3) as usize];
    let mut bitrate = BIT_RATE[0][1][bitrate_index];
    if version == 2 {
        sample_rate >>= 1;
    } else if version == 0 {
        sample_rate >>= 2;
    } else if version == 3 {
        bitrate = BIT_RATE[1][1][bitrate_index];
    }
    if sample_rate == 0 {
        return None;
    }
    Some(((144_000 * bitrate) / u32::from(sample_rate) + padded) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    fn blank() -> MpegAudioHeader {
        MpegAudioHeader {
            bitrate: 0xa5a5_a5a5,
            samplerate: 0xa5a5,
            numframes: 0xa5a5,
            framebytes: 0xa5a5,
            pad_short: 0xa5a5,
            version: 0xa5,
            layer: 0xa5,
            channels: 0xa5,
            mode: 0xa5,
            modeext: 0xa5,
            crc: 0xa5,
            padded: 0xa5,
            samplerateindex: 0xa5,
            bitrateindex: 0xa5,
            pad_char: [0xa5; 3],
        }
    }

    #[test]
    fn native_layout_is_the_24_byte_mpeg_header() {
        assert_eq!(size_of::<MpegAudioHeader>(), 24);
        assert_eq!(offset_of!(MpegAudioHeader, version), 12);
        assert_eq!(offset_of!(MpegAudioHeader, bitrateindex), 20);
        assert_eq!(offset_of!(MpegAudioHeader, pad_char), 21);
    }

    #[test]
    fn parses_version_one_and_two_legacy_headers() {
        let mut v1 = blank();
        assert_eq!(
            unsafe { parse_header(0xfffb_9000, (&mut v1 as *mut MpegAudioHeader).cast()) },
            417
        );
        assert_eq!(
            (
                v1.version,
                v1.layer,
                v1.bitrate,
                v1.samplerate,
                v1.framebytes,
                v1.numframes
            ),
            (3, 3, 128, 44_100, 417, 1152)
        );
        let mut v2 = blank();
        assert_eq!(
            unsafe { parse_header(0xfff3_9000, (&mut v2 as *mut MpegAudioHeader).cast()) },
            261
        );
        assert_eq!(
            (
                v2.version,
                v2.layer,
                v2.bitrate,
                v2.samplerate,
                v2.framebytes,
                v2.numframes
            ),
            (2, 3, 80, 22_050, 261, 576)
        );
    }

    #[test]
    fn rejected_headers_preserve_original_partial_write_order() {
        let mut invalid_sync = blank();
        assert_eq!(
            unsafe {
                parse_header(
                    0x1234_5678,
                    (&mut invalid_sync as *mut MpegAudioHeader).cast(),
                )
            },
            0
        );
        assert_eq!(invalid_sync.version, 0xa5);
        let mut invalid_rate = blank();
        assert_eq!(
            unsafe {
                parse_header(
                    0xfffb_9c00,
                    (&mut invalid_rate as *mut MpegAudioHeader).cast(),
                )
            },
            0
        );
        assert_eq!(
            (
                invalid_rate.layer,
                invalid_rate.crc,
                invalid_rate.bitrateindex,
                invalid_rate.padded,
                invalid_rate.mode,
                invalid_rate.modeext,
                invalid_rate.version,
                invalid_rate.samplerateindex
            ),
            (3, 0, 9, 0, 0, 0, 3, 3)
        );
        assert_eq!(invalid_rate.channels, 0xa5);
        assert_eq!(invalid_rate.pad_short, 0xa5a5);
        assert_eq!(invalid_rate.pad_char, [0xa5; 3]);
    }

    #[test]
    fn layer_two_helper_keeps_legacy_sync_and_table_rules() {
        assert_eq!(frame_size(0xfff5_9400), Some(480));
        assert_eq!(frame_size(0xfffb_9000), None);
        assert_eq!(frame_size(0x1234_5678), None);
        // The original sync mask rejects a low version before the v2.5 branch.
        assert_eq!(frame_size(0xffe5_9400), None);
    }
}
