//! Replays the original-C++ oracle from `Build/palette_fixture_harness/` against
//! `hp2_engine::palette::heckbert_quantize`.
//!
//! `fixtures/palette_quantize.txt` holds 1152 records (2 seeds x 6 formats x keyed x
//! nentries {1,2,16,256} x 6 sizes x mips). Each record stores the exact source
//! pixels and destination prefill, then the expected return value, the 256-entry
//! palette, and the destination (including the mip area). Every comparison is
//! exact: no PRNG is re-derived here.

use std::alloc::{Layout, alloc, dealloc};
use std::ffi::c_void;

use hp2_engine::palette::{PaletteServices, heckbert_quantize};

const FIXTURE: &str = include_str!("fixtures/palette_quantize.txt");
const EXPECTED_RECORDS: usize = 1152;
const HEADER: usize = 16;

/// Test allocator: a 16-byte header holds the requested size so `free` can
/// rebuild the layout. Mirrors `appMalloc`/`appFree` for the harness.
unsafe extern "C" fn test_malloc(size: usize, _site: u32) -> *mut c_void {
    let total = size.max(1) + HEADER;
    unsafe {
        let base = alloc(Layout::from_size_align(total, 16).unwrap());
        assert!(!base.is_null());
        (base as *mut usize).write(total);
        base.add(HEADER) as *mut c_void
    }
}

unsafe extern "C" fn test_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let base = (ptr as *mut u8).sub(HEADER);
        let total = (base as *const usize).read();
        dealloc(base, Layout::from_size_align(total, 16).unwrap());
    }
}

unsafe extern "C" fn test_unpopulated_box() {
    panic!("unpopulated box reached on a fixture input");
}

static SERVICES: PaletteServices = PaletteServices {
    malloc: test_malloc,
    free: test_free,
    unpopulated_box: test_unpopulated_box,
};

fn hex_bytes(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2));
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn field<'a>(tokens: &[&'a str], key: &str) -> &'a str {
    let prefix = format!("{key}=");
    tokens
        .iter()
        .find_map(|t| t.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("missing field {key}"))
}

fn num(tokens: &[&str], key: &str) -> i64 {
    field(tokens, key).parse().unwrap()
}

#[test]
fn palette_quantize_matches_original_cpp() {
    let mut count = 0usize;
    for line in FIXTURE.lines() {
        let tokens: Vec<&str> = line.split(' ').collect();
        assert_eq!(tokens[0], "rec");
        let fmt = num(&tokens, "fmt") as i32;
        let keyed = num(&tokens, "key") != 0;
        let nent = num(&tokens, "nent") as i32;
        let w = num(&tokens, "w") as i32;
        let h = num(&tokens, "h") as i32;
        let mips = num(&tokens, "mips") != 0;
        let keyval = u32::from_str_radix(field(&tokens, "keyval"), 16).unwrap();
        let src_bytes = hex_bytes(field(&tokens, "src"));
        let dst_in = hex_bytes(field(&tokens, "dst_in"));
        let want_ret = num(&tokens, "ret") as i32;
        let pal_hex = field(&tokens, "pal");
        let want_dst = hex_bytes(field(&tokens, "dst"));

        let bpp = [4usize, 4, 3, 2, 2, 2][fmt as usize];
        assert_eq!(src_bytes.len(), (w as usize) * (h as usize) * bpp);
        let expected_dst = if mips {
            (w as usize) * (h as usize) * 22 / 16 + 512
        } else {
            (w as usize) * (h as usize)
        };
        assert_eq!(dst_in.len(), expected_dst);
        assert_eq!(want_dst.len(), expected_dst);
        assert_eq!(pal_hex.len(), 1024);

        // The callee may free srcdata, so it must come from the same allocator.
        let src = unsafe { test_malloc(src_bytes.len(), 0) } as *mut u8;
        unsafe { std::ptr::copy_nonoverlapping(src_bytes.as_ptr(), src, src_bytes.len()) };
        let mut dst = dst_in.clone();
        let mut pal = [0u16; 256];
        // The palette is pre-filled with 0xA5A5 by the harness.
        for p in pal.iter_mut() {
            *p = 0xA5A5;
        }

        let ret = unsafe {
            heckbert_quantize(
                &SERVICES,
                src as *mut c_void,
                dst.as_mut_ptr(),
                pal.as_mut_ptr(),
                fmt,
                nent,
                w,
                h,
                keyed,
                keyval,
                mips,
            )
        };

        let ctx = line.split(" src=").next().unwrap();
        assert_eq!(ret, want_ret, "return value: {ctx}");
        for (i, p) in pal.iter().enumerate() {
            let want = u16::from_str_radix(&pal_hex[i * 4..i * 4 + 4], 16).unwrap();
            assert_eq!(*p, want, "palette[{i}]: {ctx}");
        }
        assert_eq!(dst, want_dst, "destination: {ctx}");
        count += 1;
    }
    assert_eq!(count, EXPECTED_RECORDS);
}
