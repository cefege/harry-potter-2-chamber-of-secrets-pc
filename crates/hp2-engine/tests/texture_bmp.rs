//! `UTexture::Bmp` reads the on-disk `BITMAPFILEHEADER` (14 bytes, `bfOffBits`
//! at byte 10) and `BITMAPINFOHEADER` (40 bytes). The native size plumbing used
//! an unpacked struct mirror (16 bytes), which shifted every header field.

use std::ffi::c_void;

use hp2_engine::texture_runtime::{
    TextureRuntimeArgs, TextureRuntimeData, TextureRuntimeLeaf as L, TextureRuntimeOp,
    TextureRuntimeServices, run,
};

#[derive(Default)]
struct Host {
    file: Vec<u8>,
    init: Option<(usize, usize)>,
    pixels: Vec<u8>,
    warns: Vec<(usize, usize)>,
    changed: bool,
    palette: Vec<usize>,
    published: bool,
}

unsafe extern "C" fn call(
    native: *mut c_void,
    leaf: L,
    a: usize,
    b: usize,
    out: *mut usize,
) -> i32 {
    unsafe {
        let host = &mut *native.cast::<Host>();
        *out = 0;
        match leaf {
            L::FileLoad => *out = 1,
            L::FileSentinel => host.file.push(0),
            L::ImageView => {
                let d = &mut *(b as *mut TextureRuntimeData);
                d.data = host.file.as_mut_ptr();
                d.len = host.file.len() as i32 - 1;
            }
            L::VirtualInit => {
                host.init = Some((a, b));
                host.pixels = vec![0; a * b * 4];
            }
            L::DataView => {
                let d = &mut *(b as *mut TextureRuntimeData);
                d.data = host.pixels.as_mut_ptr();
                d.len = host.pixels.len() as i32;
            }
            L::Write => {
                host.changed |=
                    a == hp2_engine::texture_runtime::TextureRuntimeField::Changed as usize
            }
            L::FactoryWarn => host.warns.push((a, b)),
            L::FactoryConstruct
            | L::FactoryPostLoad
            | L::FactoryPaletteNew
            | L::FactoryAnimationName
            | L::FactoryCompressParam
            | L::Read => {}
            L::PaletteEmpty => host.palette.clear(),
            L::PaletteAppend => host.palette.push(a),
            L::PaletteCount => *out = host.palette.len(),
            L::FactoryPalettePublish => host.published = true,
            L::FactoryResult => *out = 1,
            _ => panic!("unexpected leaf {}", leaf as i32),
        }
        0
    }
}

/// 4x2, 24-bit, uncompressed. Rows are bottom-up BGR; 12-byte rows need no padding.
fn bmp_4x2() -> (Vec<u8>, [[u8; 3]; 8]) {
    let rows_bottom_up: [[u8; 3]; 8] = [
        [1, 2, 3],
        [4, 5, 6],
        [7, 8, 9],
        [10, 11, 12],
        [13, 14, 15],
        [16, 17, 18],
        [19, 20, 21],
        [22, 23, 24],
    ];
    let mut f = Vec::new();
    f.extend_from_slice(b"BM");
    f.extend_from_slice(&(54u32 + 24).to_le_bytes());
    f.extend_from_slice(&[0, 0, 0, 0]);
    f.extend_from_slice(&54u32.to_le_bytes());
    f.extend_from_slice(&40u32.to_le_bytes());
    f.extend_from_slice(&4i32.to_le_bytes());
    f.extend_from_slice(&2i32.to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&24u16.to_le_bytes());
    f.extend_from_slice(&0u32.to_le_bytes());
    f.extend_from_slice(&24u32.to_le_bytes());
    f.extend_from_slice(&[0; 16]);
    assert_eq!(f.len(), 54);
    for p in rows_bottom_up {
        f.extend_from_slice(&p);
    }
    (f, rows_bottom_up)
}

#[test]
fn bmp_import_reads_on_disk_header_layout() {
    let (file, src) = bmp_4x2();
    let mut host = Host {
        file,
        ..Host::default()
    };
    let services = TextureRuntimeServices { call };
    let args = TextureRuntimeArgs {
        native: (&raw mut host).cast(),
        a: 0,
        b: 0,
        time: 0,
    };
    assert_eq!(run(&services, args, TextureRuntimeOp::Bmp), Ok(1));
    assert!(host.warns.is_empty(), "warnings {:?}", host.warns);
    assert!(host.changed);
    // Texture size is `1 << (32 - leading_zeros(n))`: 4x2 imports into 8x4.
    assert_eq!(host.init, Some((8, 4)));
    // File rows are bottom-up; BGR bytes copy through with alpha 255 at stride 8.
    let mut expected = vec![0u8; 8 * 4 * 4];
    for y in 0..2 {
        for x in 0..4 {
            let at = ((1 - y) * 8 + x) * 4;
            expected[at..at + 3].copy_from_slice(&src[y * 4 + x]);
            expected[at + 3] = 255;
        }
    }
    assert_eq!(host.pixels, expected);
}

/// 4x2, 8-bit indexed, `biClrUsed = 2`. Palette entries are BGRx at byte 54.
fn bmp_indexed_4x2() -> (Vec<u8>, [u8; 8]) {
    let indices: [u8; 8] = [0, 1, 1, 0, 1, 0, 0, 1];
    let mut f = Vec::new();
    f.extend_from_slice(b"BM");
    f.extend_from_slice(&(62u32 + 8).to_le_bytes());
    f.extend_from_slice(&[0, 0, 0, 0]);
    f.extend_from_slice(&62u32.to_le_bytes());
    f.extend_from_slice(&40u32.to_le_bytes());
    f.extend_from_slice(&4i32.to_le_bytes());
    f.extend_from_slice(&2i32.to_le_bytes());
    f.extend_from_slice(&1u16.to_le_bytes());
    f.extend_from_slice(&8u16.to_le_bytes());
    f.extend_from_slice(&0u32.to_le_bytes());
    f.extend_from_slice(&8u32.to_le_bytes());
    f.extend_from_slice(&[0; 8]);
    f.extend_from_slice(&2u32.to_le_bytes());
    f.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(f.len(), 54);
    f.extend_from_slice(&[0x10, 0x20, 0x30, 0, 0x40, 0x50, 0x60, 0]);
    f.extend_from_slice(&indices);
    (f, indices)
}

#[test]
fn indexed_bmp_factory_reads_palette_after_on_disk_headers() {
    let (file, indices) = bmp_indexed_4x2();
    let mut host = Host {
        file,
        ..Host::default()
    };
    // The mock's `ImageView` excludes one trailing byte, as the native file path does.
    host.file.push(0);
    let services = TextureRuntimeServices { call };
    let args = TextureRuntimeArgs {
        native: (&raw mut host).cast(),
        a: 0,
        b: 0,
        time: 0,
    };
    assert_eq!(run(&services, args, TextureRuntimeOp::Factory), Ok(1));
    assert!(host.warns.is_empty(), "warnings {:?}", host.warns);
    assert_eq!(host.init, Some((4, 2)));
    assert!(host.published);
    // `FColor(R=pal[2], G=pal[1], B=pal[0], 255)`, R in the low byte.
    let mut expected = vec![0xff10_2030usize, 0xff40_5060];
    expected.resize(256, 0xff00_0000);
    assert_eq!(host.palette, expected);
    // Bottom-up rows land top-down at the texture width.
    assert_eq!(
        &host.pixels[..8],
        &[
            indices[4], indices[5], indices[6], indices[7], indices[0], indices[1], indices[2],
            indices[3]
        ]
    );
}
