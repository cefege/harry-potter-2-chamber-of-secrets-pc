//! Heckbert colour quantiser: Rust port of `HarryPotter2/Unreal/Engine/Src/palette.cpp`
//! (pinned `452b122`). Bit-exact with the C++ at -O3 (integer-only code, no FMA sites).
//!
//! Divergences, all documented at their sites:
//! - `nentries > 256` returns 0 up front (the C++ writes past the static `boxlist[256]`).
//! - The C++ statics (`histogram`, `in_pal`, `boxlist`, colour count) are a fresh zeroed
//!   `State` per call. The C++ memsets `histogram` and `boxlist` on every call and
//!   `in_pal` whenever mipmaps are requested; `in_pal` is only read inside
//!   `GenerateMipmap`, so zeroing it unconditionally is not observable.
//! - Colour weightings are the constants the dead `SetupColourWeightings` would have set
//!   (`ascale=400 rscale=200 gscale=300 bscale=100`).
//! - `GenerateMipmap` and `CopyPalettisedData` index raw pointers with the exact C++
//!   arithmetic, including the odd-dimension over-reads and the over-writes the C++
//!   performs. The harness pre-fills the whole mip buffer deterministically so the replay
//!   sees identical bytes past the level.

use std::ffi::c_void;

pub const ARGB_8888: i32 = 0;
pub const XRGB_8888: i32 = 1;
pub const RGB_888: i32 = 2;
pub const RGB_565: i32 = 3;
pub const ARGB_1555: i32 = 4;
pub const ARGB_4444: i32 = 5;

/// Allocation site ids passed to `PaletteServices::malloc`. The C++ thunk maps them to the
/// `appMalloc` tag strings the original used.
pub const SITE_HISTOGRAM_8888: u32 = 0;
pub const SITE_HISTOGRAM_X888: u32 = 1;
pub const SITE_HISTOGRAM_888: u32 = 2;

/// C++ services (`Hp2PaletteServices` in `Hp2PaletteServices.cpp`), same field order.
#[repr(C)]
pub struct PaletteServices {
    /// `appMalloc(appCheckedAllocSize(size), tag_for(site))`.
    pub malloc: unsafe extern "C" fn(size: usize, site: u32) -> *mut c_void,
    /// `appFree(ptr)`.
    pub free: unsafe extern "C" fn(ptr: *mut c_void),
    /// `debugf(TEXT("Unpopulated box!\n"))`.
    pub unpopulated_box: unsafe extern "C" fn(),
}

const HIST_LEN: usize = 65536;
const MAX_BOXES: usize = 256;

const ASCALE: u32 = 400;
const RSCALE: u32 = 200;
const GSCALE: u32 = 300;
const BSCALE: u32 = 100;

#[derive(Clone, Copy, Default)]
struct BoxT {
    amin: i32,
    amax: i32,
    rmin: i32,
    rmax: i32,
    gmin: i32,
    gmax: i32,
    bmin: i32,
    bmax: i32,
    volume: i32,
    colour_count: i32,
}

/// Per-call replacement for the C++ file statics.
struct State<'a> {
    svc: &'a PaletteServices,
    histogram: Vec<u32>,
    in_pal: Vec<u32>,
    colour_count: i32,
    alpha_data: bool,
    boxlist: [BoxT; MAX_BOXES],
    amul: i32,
    rmul: i32,
    gmul: i32,
    bmul: i32,
    ashift: i32,
    rshift: i32,
    gshift: i32,
    bshift: i32,
}

impl<'a> State<'a> {
    fn new(svc: &'a PaletteServices) -> Self {
        Self {
            svc,
            histogram: vec![0; HIST_LEN],
            in_pal: vec![0; HIST_LEN],
            colour_count: 0,
            alpha_data: false,
            boxlist: [BoxT::default(); MAX_BOXES],
            amul: 0,
            rmul: 0,
            gmul: 0,
            bmul: 0,
            ashift: 0,
            rshift: 0,
            gshift: 0,
            bshift: 0,
        }
    }

    /// Bumps `histogram[item]`, counting a new colour on its first hit.
    #[inline]
    fn bump(&mut self, item: usize) {
        if self.histogram[item] == 0 {
            self.colour_count += 1;
        }
        self.histogram[item] += 1;
    }
}

// ---------------------------------------------------------------------------
// Histograms
// ---------------------------------------------------------------------------

/// `Histogram8888`: ARGB_8888 source, returns the 16-bit quantised colour per pixel.
unsafe fn histogram_8888(st: &mut State, data: *const u32, width: i32, height: i32) -> *mut u16 {
    // `i = width * height` is an `int` product, then `static_cast<SIZE_T>(i)`.
    let n = width.wrapping_mul(height) as isize as usize;
    let qd = unsafe { (st.svc.malloc)(n.wrapping_mul(2), SITE_HISTOGRAM_8888) } as *mut u16;
    let mut i = n;
    while i != 0 {
        i -= 1;
        let a = unsafe { *data.add(i) };
        let sum = ((a & 0xf000_0000) >> 16)
            .wrapping_add((a & 0x00f0_0000) >> 12)
            .wrapping_add((a & 0x0000_f000) >> 8)
            .wrapping_add((a & 0x0000_00f0) >> 4);
        let item = (sum as u16) as usize;
        unsafe { *qd.add(i) = item as u16 };
        st.bump(item);
    }
    qd
}

/// `HistogramX888`: XRGB_8888 source (RGB_565 quantisation).
unsafe fn histogram_x888(st: &mut State, data: *const u32, width: i32, height: i32) -> *mut u16 {
    // `i = width * height` is an `int` product, then `static_cast<SIZE_T>(i)`.
    let n = width.wrapping_mul(height) as isize as usize;
    let qd = unsafe { (st.svc.malloc)(n.wrapping_mul(2), SITE_HISTOGRAM_X888) } as *mut u16;
    let mut i = n;
    while i != 0 {
        i -= 1;
        let a = unsafe { *data.add(i) };
        let sum = ((a & 0x00f8_0000) >> 8)
            .wrapping_add((a & 0x0000_fc00) >> 5)
            .wrapping_add((a & 0x0000_00f8) >> 3);
        let item = (sum as u16) as usize;
        unsafe { *qd.add(i) = item as u16 };
        st.bump(item);
    }
    qd
}

/// `Histogram888`: RGB_888 source (bytes, 3 per pixel), RGB_565 quantisation.
unsafe fn histogram_888(st: &mut State, data: *const u8, width: i32, height: i32) -> *mut u16 {
    let n = (width as usize).wrapping_mul(height as usize);
    let qd = unsafe { (st.svc.malloc)(n.wrapping_mul(2), SITE_HISTOGRAM_888) } as *mut u16;
    let mut k = 0usize;
    let mut j = 0usize;
    while j < n.wrapping_mul(3) {
        let r = (unsafe { *data.add(j) } & 0xf8) as u32;
        let g = (unsafe { *data.add(j + 1) } & 0xfc) as u32;
        let b = (unsafe { *data.add(j + 2) } & 0xf8) as u32;
        let item = (((r << 8) | (g << 3) | (b >> 3)) as u16) as usize;
        unsafe { *qd.add(k) = item as u16 };
        st.bump(item);
        j += 3;
        k += 1;
    }
    qd
}

/// `Histogram16Bit`: 16-bit sources are used as their own quantised colour array (the
/// returned pointer IS `data`). The C++ later `appFree`s that pointer, so the caller's
/// buffer is freed by `heckbert_quantize`. Reproduced.
unsafe fn histogram_16bit(
    st: &mut State,
    data: *mut u16,
    width: i32,
    height: i32,
    format: i32,
) -> *mut u16 {
    let n = (width as usize).wrapping_mul(height as usize);
    let mut i = n;
    while i != 0 {
        i -= 1;
        let item = unsafe { *data.add(i) } as usize;
        match format {
            RGB_565 | ARGB_1555 | ARGB_4444 => st.bump(item),
            _ => {}
        }
    }
    data
}

// ---------------------------------------------------------------------------
// Box search
// ---------------------------------------------------------------------------

fn find_most_populated_box(boxlist: &[BoxT; MAX_BOXES], n: i32) -> Option<usize> {
    let mut maxc = 0i32;
    let mut which = None;
    for (i, b) in boxlist.iter().enumerate().take(n as usize) {
        if b.colour_count > maxc && b.volume > 0 {
            which = Some(i);
            maxc = b.colour_count;
        }
    }
    which
}

fn find_box_with_largest_volume(boxlist: &[BoxT; MAX_BOXES], n: i32) -> Option<usize> {
    let mut maxv = 0i32;
    let mut which = None;
    for (i, b) in boxlist.iter().enumerate().take(n as usize) {
        if b.volume > maxv {
            which = Some(i);
            maxv = b.volume;
        }
    }
    which
}

/// `UpdateBoxInfo` / `UpdateBoxInfoWithAlpha` (selected by `st.alpha_data`).
///
/// The C++ shrinks each face in order (amin, amax, rmin, rmax, gmin, gmax, bmin, bmax);
/// each successful scan jumps to the next face (`goto`). Ported as sequential labelled
/// blocks, which is the same control flow. The non-alpha variant has no alpha term and
/// no alpha faces; it uses `a = 0` with the alpha shift term suppressed.
fn update_box_info(st: &mut State, idx: usize) {
    let alpha = st.alpha_data;
    let bx = st.boxlist[idx];
    let (mut amin, mut amax) = (bx.amin, bx.amax);
    let (mut rmin, mut rmax) = (bx.rmin, bx.rmax);
    let (mut gmin, mut gmax) = (bx.gmin, bx.gmax);
    let (mut bmin, mut bmax) = (bx.bmin, bx.bmax);
    let (ashift, rshift, gshift, bmul) = (st.ashift, st.rshift, st.gshift, st.bmul);
    let hist = &st.histogram;

    let at = |a: i32| if alpha { a << ashift } else { 0 };
    let arange =
        |lo: i32, hi: i32| -> std::ops::RangeInclusive<i32> { if alpha { lo..=hi } else { 0..=0 } };

    // amin
    'amin: {
        if alpha && amax > amin {
            for a in amin..=amax {
                for r in rmin..=rmax {
                    for g in gmin..=gmax {
                        for b in bmin..=bmax {
                            let i = at(a) + (r << rshift) + (g << gshift) + b;
                            if hist[i as usize] != 0 {
                                amin = a;
                                break 'amin;
                            }
                        }
                    }
                }
            }
        }
    }
    // amax
    'amax: {
        if alpha && amax > amin {
            for a in (amin..=amax).rev() {
                for r in rmin..=rmax {
                    for g in gmin..=gmax {
                        for b in bmin..=bmax {
                            let i = at(a) + (r << rshift) + (g << gshift) + b;
                            if hist[i as usize] != 0 {
                                amax = a;
                                break 'amax;
                            }
                        }
                    }
                }
            }
        }
    }
    // rmin
    'rmin: {
        if rmax > rmin {
            for r in rmin..=rmax {
                for a in arange(amin, amax) {
                    for g in gmin..=gmax {
                        for b in bmin..=bmax {
                            let i = at(a) + (r << rshift) + (g << gshift) + b;
                            if hist[i as usize] != 0 {
                                rmin = r;
                                break 'rmin;
                            }
                        }
                    }
                }
            }
        }
    }
    // rmax
    'rmax: {
        if rmax > rmin {
            for r in (rmin..=rmax).rev() {
                for a in arange(amin, amax) {
                    for g in gmin..=gmax {
                        for b in bmin..=bmax {
                            let i = at(a) + (r << rshift) + (g << gshift) + b;
                            if hist[i as usize] != 0 {
                                rmax = r;
                                break 'rmax;
                            }
                        }
                    }
                }
            }
        }
    }
    // gmin
    'gmin: {
        if gmax > gmin {
            for g in gmin..=gmax {
                for a in arange(amin, amax) {
                    for r in rmin..=rmax {
                        for b in bmin..=bmax {
                            let i = at(a) + (r << rshift) + (g << gshift) + b;
                            if hist[i as usize] != 0 {
                                gmin = g;
                                break 'gmin;
                            }
                        }
                    }
                }
            }
        }
    }
    // gmax
    'gmax: {
        if gmax > gmin {
            for g in (gmin..=gmax).rev() {
                for a in arange(amin, amax) {
                    for r in rmin..=rmax {
                        for b in bmin..=bmax {
                            let i = at(a) + (r << rshift) + (g << gshift) + b;
                            if hist[i as usize] != 0 {
                                gmax = g;
                                break 'gmax;
                            }
                        }
                    }
                }
            }
        }
    }
    // bmin: histdata steps by (bmul+1) per g, starting at g = gmin.
    'bmin: {
        if bmax > bmin {
            for b in bmin..=bmax {
                for a in arange(amin, amax) {
                    for r in rmin..=rmax {
                        for g in gmin..=gmax {
                            let i = at(a)
                                + (r << rshift)
                                + (gmin << gshift)
                                + b
                                + (g - gmin) * (bmul + 1);
                            if hist[i as usize] != 0 {
                                bmin = b;
                                break 'bmin;
                            }
                        }
                    }
                }
            }
        }
    }
    // bmax
    'bmax: {
        if bmax > bmin {
            for b in (bmin..=bmax).rev() {
                for a in arange(amin, amax) {
                    for r in rmin..=rmax {
                        for g in gmin..=gmax {
                            let i = at(a)
                                + (r << rshift)
                                + (gmin << gshift)
                                + b
                                + (g - gmin) * (bmul + 1);
                            if hist[i as usize] != 0 {
                                bmax = b;
                                break 'bmax;
                            }
                        }
                    }
                }
            }
        }
    }

    // Volume (DWORD arithmetic, stored as INT).
    let volume: u32 = if alpha {
        let d0 = ((amax - amin) as u32).wrapping_mul(ASCALE);
        let d1 = ((rmax - rmin) as u32).wrapping_mul(RSCALE);
        let d2 = ((gmax - gmin) as u32).wrapping_mul(GSCALE);
        let d3 = ((bmax - bmin) as u32).wrapping_mul(BSCALE);
        d0.wrapping_mul(d0)
            .wrapping_add(d1.wrapping_mul(d1))
            .wrapping_add(d2.wrapping_mul(d2))
            .wrapping_add(d3.wrapping_mul(d3))
    } else {
        let d0 = ((rmax - rmin) as u32).wrapping_mul(RSCALE);
        let d1 = ((gmax - gmin) as u32).wrapping_mul(GSCALE);
        let d2 = ((bmax - bmin) as u32).wrapping_mul(BSCALE);
        d0.wrapping_mul(d0)
            .wrapping_add(d1.wrapping_mul(d1))
            .wrapping_add(d2.wrapping_mul(d2))
    };

    // Population over the whole box.
    let mut ccount = 0i32;
    for a in arange(amin, amax) {
        for r in rmin..=rmax {
            for g in gmin..=gmax {
                for b in bmin..=bmax {
                    let i = at(a) + (r << rshift) + (g << gshift) + b;
                    if hist[i as usize] != 0 {
                        ccount += 1;
                    }
                }
            }
        }
    }
    if ccount == 0 {
        unsafe { (st.svc.unpopulated_box)() };
    }

    let bptr = &mut st.boxlist[idx];
    bptr.amin = amin;
    bptr.amax = amax;
    bptr.rmin = rmin;
    bptr.rmax = rmax;
    bptr.gmin = gmin;
    bptr.gmax = gmax;
    bptr.bmin = bmin;
    bptr.bmax = bmax;
    bptr.volume = volume as i32;
    bptr.colour_count = ccount;
}

// ---------------------------------------------------------------------------
// Palette construction
// ---------------------------------------------------------------------------

/// `ComputePalette`: average each box's colours, assign palette indices.
fn compute_palette(st: &mut State, paldata: *mut u16, nboxes: i32, has_key: bool) {
    let mut nboxes = nboxes;
    let mut boffset = 0i32;
    if has_key {
        nboxes -= 1;
        boffset = 1;
    }
    for i in 0..nboxes {
        let bx = st.boxlist[i as usize];
        let mut total = 0u32;
        let mut rtotal = 0u32;
        let mut gtotal = 0u32;
        let mut btotal = 0u32;
        for r in bx.rmin..=bx.rmax {
            for g in bx.gmin..=bx.gmax {
                for b in bx.bmin..=bx.bmax {
                    let idx = ((r << st.rshift) + (g << st.gshift) + b) as usize;
                    let count = st.histogram[idx] as i32;
                    if count != 0 {
                        total = total.wrapping_add(count as u32);
                        rtotal = rtotal.wrapping_add(r.wrapping_mul(count) as u32);
                        gtotal = gtotal.wrapping_add(g.wrapping_mul(count) as u32);
                        btotal = btotal.wrapping_add(b.wrapping_mul(count) as u32);
                    }
                    st.histogram[idx] = (i + boffset) as u32;
                    st.in_pal[idx] = 1;
                }
            }
        }
        let value: u16 = match total {
            0 => 0,
            _ => {
                let r = (rtotal / total) as i32;
                let g = (gtotal / total) as i32;
                let b = (btotal / total) as i32;
                ((r << st.rshift) | (g << st.gshift) | (b << st.bshift)) as u16
            }
        };
        unsafe { *paldata.add((i + boffset) as usize) = value };
    }
}

/// `ComputeAlphaPalette`: as `compute_palette` over ARGB, no palette offset.
fn compute_alpha_palette(st: &mut State, paldata: *mut u16, nboxes: i32) {
    for i in 0..nboxes {
        let bx = st.boxlist[i as usize];
        let mut total = 0u32;
        let mut atotal = 0u32;
        let mut rtotal = 0u32;
        let mut gtotal = 0u32;
        let mut btotal = 0u32;
        for a in bx.amin..=bx.amax {
            for r in bx.rmin..=bx.rmax {
                for g in bx.gmin..=bx.gmax {
                    for b in bx.bmin..=bx.bmax {
                        let idx =
                            ((a << st.ashift) + (r << st.rshift) + (g << st.gshift) + b) as usize;
                        let count = st.histogram[idx] as i32;
                        if count != 0 {
                            total = total.wrapping_add(count as u32);
                            atotal = atotal.wrapping_add(a.wrapping_mul(count) as u32);
                            rtotal = rtotal.wrapping_add(r.wrapping_mul(count) as u32);
                            gtotal = gtotal.wrapping_add(g.wrapping_mul(count) as u32);
                            btotal = btotal.wrapping_add(b.wrapping_mul(count) as u32);
                        }
                        st.histogram[idx] = i as u32;
                        st.in_pal[idx] = 1;
                    }
                }
            }
        }
        let value: u16 = match total {
            0 => 0,
            _ => {
                let a = (atotal / total) as i32;
                let r = (rtotal / total) as i32;
                let g = (gtotal / total) as i32;
                let b = (btotal / total) as i32;
                ((a << st.ashift) | (r << st.rshift) | (g << st.gshift) | (b << st.bshift)) as u16
            }
        };
        unsafe { *paldata.add(i as usize) = value };
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// `CopyPalettisedData`: writes palette indices into `dstdata`, applies the colour key.
/// Returns 0 for a keyed unknown format, else 1 (the caller ignores it).
#[allow(clippy::too_many_arguments)]
unsafe fn copy_palettised_data(
    st: &State,
    orig_data: *const c_void,
    qd: *const u16,
    dstdata: *mut u8,
    srcformat: i32,
    width: i32,
    height: i32,
    has_key: bool,
    keyval: u32,
) -> i32 {
    let n = (width as usize).wrapping_mul(height as usize);
    for i in 0..n {
        let idx = unsafe { *qd.add(i) } as usize;
        unsafe { *dstdata.add(i) = st.histogram[idx] as u8 };
    }
    if has_key {
        match srcformat {
            XRGB_8888 => {
                let data = orig_data as *const u32;
                for i in 0..n {
                    if unsafe { *data.add(i) } == keyval {
                        unsafe { *dstdata.add(i) = 0 };
                    }
                }
            }
            RGB_888 => {
                let data = orig_data as *const u8;
                let mut j = 0usize;
                for k in 0..n {
                    let r = unsafe { *data.add(j) } as u32;
                    let g = unsafe { *data.add(j + 1) } as u32;
                    let b = unsafe { *data.add(j + 2) } as u32;
                    j += 3;
                    let item = (r << 16) | (g << 8) | b;
                    if item == keyval {
                        unsafe { *dstdata.add(k) = 0 };
                    }
                }
            }
            RGB_565 => {
                let data = orig_data as *const u16;
                for i in 0..n {
                    if unsafe { *data.add(i) } as u32 == keyval {
                        unsafe { *dstdata.add(i) = 0 };
                    }
                }
            }
            _ => return 0,
        }
    }
    1
}

/// `IndexSearch`: nearest palette entry to `colour`; records it in `in_pal`/`histogram`.
unsafe fn index_search(
    st: &mut State,
    colour: u32,
    palette: *const u16,
    pal_format: i32,
    has_key: bool,
    nindices: i32,
) -> i32 {
    let mut best = 0i32;
    let mut mindist = 500_000i32;
    let acomp4444 = |c: i32| (c & 0xf000) >> 8;
    let rcomp4444 = |c: i32| (c & 0x0f00) >> 4;
    let gcomp4444 = |c: i32| c & 0x00f0;
    let bcomp4444 = |c: i32| (c & 0x000f) << 4;
    let acomp1555 = |c: i32| if c & 0x8000 != 0 { 0xff } else { 0 };
    let rcomp1555 = |c: i32| (c & 0x7c00) >> 7;
    let gcomp1555 = |c: i32| (c & 0x03e0) >> 2;
    let bcomp1555 = |c: i32| (c & 0x001f) << 3;
    let rcomp565 = |c: i32| (c & 0xf800) >> 8;
    let gcomp565 = |c: i32| (c & 0x07e0) >> 3;
    let bcomp565 = |c: i32| (c & 0x001f) << 3;

    let colour_i = colour as i32;
    match pal_format {
        ARGB_4444 => {
            let a = acomp4444(colour_i);
            let r = rcomp4444(colour_i);
            let g = gcomp4444(colour_i);
            let b = bcomp4444(colour_i);
            for i in 0..nindices {
                let pc = unsafe { *palette.add(i as usize) } as i32;
                let ad = (a - acomp4444(pc)).abs();
                let rd = (r - rcomp4444(pc)).abs();
                let gd = (g - gcomp4444(pc)).abs();
                let bd = (b - bcomp4444(pc)).abs();
                let dist = ad + ad + rd + gd + bd;
                if dist >= mindist {
                    continue;
                }
                if dist == 0 {
                    best = i;
                    break;
                }
                mindist = dist;
                best = i;
            }
        }
        ARGB_1555 => {
            let a = acomp1555(colour_i);
            let r = rcomp1555(colour_i);
            let g = gcomp1555(colour_i);
            let b = bcomp1555(colour_i);
            for i in 0..nindices {
                let pc = unsafe { *palette.add(i as usize) } as i32;
                let ad = (a - acomp1555_i(pc)).abs();
                if ad != 0 {
                    continue;
                }
                let rd = (r - rcomp1555(pc)).abs();
                let gd = (g - gcomp1555(pc)).abs();
                let bd = (b - bcomp1555(pc)).abs();
                let dist = rd + gd + bd;
                if dist >= mindist {
                    continue;
                }
                if dist == 0 {
                    best = i;
                    break;
                }
                mindist = dist;
                best = i;
            }
        }
        _ => {
            let r = rcomp565(colour_i);
            let g = gcomp565(colour_i);
            let b = bcomp565(colour_i);
            let mut i = if has_key { 1 } else { 0 };
            while i < nindices {
                let pc = unsafe { *palette.add(i as usize) } as i32;
                let rd = (r - rcomp565(pc)).abs();
                let gd = (g - gcomp565(pc)).abs();
                let bd = (b - bcomp565(pc)).abs();
                let dist = rd + gd + bd;
                if dist < mindist {
                    if dist == 0 {
                        best = i;
                        break;
                    }
                    mindist = dist;
                    best = i;
                }
                i += 1;
            }
        }
    }
    st.in_pal[colour as usize] = 1;
    st.histogram[colour as usize] = best as u32;
    1
}

/// `GenerateMipmap`: builds the next level from `srcdata` into `dstdata` (recursive).
///
/// Quirk reproduced: when a new palette entry is added (`nboxes < maxboxes`), the C++ does
/// NOT write `out[x>>1]` on that path. Only the IndexSearch path and the `in_pal`-hit path
/// write it. Odd dimensions read/write past the row exactly as the C++ does; the mip
/// buffer is sized by the caller and pre-filled deterministically by the harness.
#[allow(clippy::too_many_arguments)]
unsafe fn generate_mipmap(
    st: &mut State,
    srcdata: *mut u8,
    dstdata: *mut u8,
    palette: *mut u16,
    pal_format: i32,
    width: i32,
    height: i32,
    has_key: bool,
    nboxes: i32,
    maxboxes: i32,
) -> i32 {
    let mut nboxes = nboxes;
    if width < 2 {
        return nboxes;
    }
    if height < 2 {
        return nboxes;
    }
    let w = width as usize;
    let h = height as usize;
    let hw = w >> 1;

    let mut y = 0usize;
    while y < h {
        let inp = unsafe { srcdata.add(y * w) };
        let out = unsafe { dstdata.add((y >> 1) * hw) };
        let mut x = 0usize;
        while x < w {
            // Over-reads past the row/level for odd `w` are reproduced (wrapping_add).
            let i1 = unsafe { *inp.wrapping_add(x) } as usize;
            let i2 = unsafe { *inp.wrapping_add(x + 1) } as usize;
            let i3 = unsafe { *inp.wrapping_add(x + w) } as usize;
            let i4 = unsafe { *inp.wrapping_add(x + w + 1) } as usize;

            // Colour-key test for 565 only (the 16-bit ARGB paths never reach it).
            let mut keyed_out = false;
            if pal_format != ARGB_4444 && pal_format != ARGB_1555 && has_key {
                let cktest =
                    (i1 == 0) as i32 + (i2 == 0) as i32 + (i3 == 0) as i32 + (i4 == 0) as i32;
                if cktest > 2 {
                    keyed_out = true;
                }
            }
            if keyed_out {
                unsafe { *out.add(x >> 1) = 0 };
                x += 2;
                continue;
            }

            let c1 = unsafe { *palette.add(i1) } as i32;
            let c2 = unsafe { *palette.add(i2) } as i32;
            let c3 = unsafe { *palette.add(i3) } as i32;
            let c4 = unsafe { *palette.add(i4) } as i32;

            let texel: u32 = match pal_format {
                ARGB_4444 => {
                    let a = (c1 & 0xf000) >> 8;
                    let a = a + ((c2 & 0xf000) >> 8);
                    let a = a + ((c3 & 0xf000) >> 8);
                    let a = (a + ((c4 & 0xf000) >> 8)) >> 2;
                    let r = (c1 & 0x0f00) >> 4;
                    let r = r + ((c2 & 0x0f00) >> 4);
                    let r = r + ((c3 & 0x0f00) >> 4);
                    let r = (r + ((c4 & 0x0f00) >> 4)) >> 2;
                    let g = c1 & 0x00f0;
                    let g = g + (c2 & 0x00f0);
                    let g = g + (c3 & 0x00f0);
                    let g = (g + (c4 & 0x00f0)) >> 2;
                    let b = (c1 & 0x000f) << 4;
                    let b = b + ((c2 & 0x000f) << 4);
                    let b = b + ((c3 & 0x000f) << 4);
                    let b = (b + ((c4 & 0x000f) << 4)) >> 2;
                    ((((a & 0xf0) << 8) | ((r & 0xf0) << 4) | (g & 0xf0) | ((b & 0xf0) >> 4))
                        as u16) as u32
                }
                ARGB_1555 => {
                    let a = acomp1555_i(c1) + acomp1555_i(c2) + acomp1555_i(c3) + acomp1555_i(c4);
                    let a = a >> 2;
                    let r = rcomp1555_i(c1) + rcomp1555_i(c2) + rcomp1555_i(c3) + rcomp1555_i(c4);
                    let r = r >> 2;
                    let g = gcomp1555_i(c1) + gcomp1555_i(c2) + gcomp1555_i(c3) + gcomp1555_i(c4);
                    let g = g >> 2;
                    let b = bcomp1555_i(c1) + bcomp1555_i(c2) + bcomp1555_i(c3) + bcomp1555_i(c4);
                    let b = b >> 2;
                    let t = (((r & 0xf8) << 7) | ((g & 0xf8) << 2) | ((b & 0xf8) >> 3)) as u16;
                    if a > 0x80 {
                        (0x8000 | t) as u32
                    } else {
                        t as u32
                    }
                }
                _ => {
                    let r = rcomp565_i(c1) + rcomp565_i(c2) + rcomp565_i(c3) + rcomp565_i(c4);
                    let r = r >> 2;
                    let g = gcomp565_i(c1) + gcomp565_i(c2) + gcomp565_i(c3) + gcomp565_i(c4);
                    let g = g >> 2;
                    let b = bcomp565_i(c1) + bcomp565_i(c2) + bcomp565_i(c3) + bcomp565_i(c4);
                    let b = b >> 2;
                    (((r & 0xf8) << 8) | ((g & 0xfc) << 3) | ((b & 0xf8) >> 3)) as u16 as u32
                }
            };

            if st.in_pal[texel as usize] == 0 {
                if nboxes < maxboxes {
                    // New entry: the C++ does not write out[x>>1] on this path.
                    unsafe { *palette.add(nboxes as usize) = texel as u16 };
                    st.in_pal[texel as usize] = 1;
                    st.histogram[texel as usize] = nboxes as u32;
                    nboxes += 1;
                } else {
                    unsafe { index_search(st, texel, palette, pal_format, has_key, nboxes) };
                    unsafe { *out.add(x >> 1) = st.histogram[texel as usize] as u8 };
                }
            } else {
                unsafe { *out.add(x >> 1) = st.histogram[texel as usize] as u8 };
            }
            x += 2;
        }
        y += 2;
    }

    // Recurse to the next level.
    unsafe {
        generate_mipmap(
            st,
            srcdata.wrapping_add(w * h),
            dstdata.wrapping_add(hw * (h >> 1)),
            palette,
            pal_format,
            width >> 1,
            height >> 1,
            has_key,
            nboxes,
            maxboxes,
        )
    }
}

// Component extractors for the 16-bit palette colours (C++ macros, int results).
#[inline]
fn acomp1555_i(c: i32) -> i32 {
    if c & 0x8000 != 0 { 0xff } else { 0 }
}
#[inline]
fn rcomp1555_i(c: i32) -> i32 {
    (c & 0x7c00) >> 7
}
#[inline]
fn gcomp1555_i(c: i32) -> i32 {
    (c & 0x03e0) >> 2
}
#[inline]
fn bcomp1555_i(c: i32) -> i32 {
    (c & 0x001f) << 3
}
#[inline]
fn rcomp565_i(c: i32) -> i32 {
    (c & 0xf800) >> 8
}
#[inline]
fn gcomp565_i(c: i32) -> i32 {
    (c & 0x07e0) >> 3
}
#[inline]
fn bcomp565_i(c: i32) -> i32 {
    (c & 0x001f) << 3
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// `HeckbertQuantize`. Returns the number of palette entries (`nboxes`), or 0 for an
/// unknown format, for `has_colourkey && nentries <= 1`, or for `nentries > 256`.
///
/// Quirk reproduced: the `has_colourkey && nentries <= 1` early return leaves the
/// quantise buffer allocated (the C++ never frees it). Divergence: `nentries > 256`
/// returns 0 before any writes instead of overflowing the static box list.
///
/// # Safety
/// `srcdata` must be valid for the source format's full `width*height` pixels (3 bytes
/// per pixel for RGB_888). `dstdata` must be valid for the level-0 output plus the mip
/// chain (the C++ over-writes up to one row past each level). `paldata` must hold
/// `nentries` `u16`. `svc` must be valid.
#[allow(clippy::too_many_arguments)]
pub unsafe fn heckbert_quantize(
    svc: &PaletteServices,
    srcdata: *mut c_void,
    dstdata: *mut u8,
    paldata: *mut u16,
    srcformat: i32,
    nentries: i32,
    width: i32,
    height: i32,
    has_colourkey: bool,
    keyval: u32,
    generate_mipmaps: bool,
) -> i32 {
    if nentries > MAX_BOXES as i32 {
        return 0;
    }
    let mut st = State::new(svc);
    st.alpha_data = false;
    let mut has_key = has_colourkey;
    let mut nentries = nentries;

    let quantise_data: *mut u16;
    let hist_format: i32;
    match srcformat {
        ARGB_8888 => {
            quantise_data =
                unsafe { histogram_8888(&mut st, srcdata as *const u32, width, height) };
            st.alpha_data = true;
            hist_format = ARGB_4444;
            st.amul = 15;
            st.rmul = 15;
            st.gmul = 15;
            st.bmul = 15;
            st.ashift = 12;
            st.rshift = 8;
            st.gshift = 4;
            st.bshift = 0;
        }
        XRGB_8888 => {
            quantise_data =
                unsafe { histogram_x888(&mut st, srcdata as *const u32, width, height) };
            hist_format = RGB_565;
            st.amul = 0;
            st.rmul = 31;
            st.gmul = 63;
            st.bmul = 31;
            st.ashift = 0;
            st.rshift = 11;
            st.gshift = 5;
            st.bshift = 0;
        }
        RGB_888 => {
            quantise_data = unsafe { histogram_888(&mut st, srcdata as *const u8, width, height) };
            hist_format = RGB_565;
            st.amul = 0;
            st.rmul = 31;
            st.gmul = 63;
            st.bmul = 31;
            st.ashift = 0;
            st.rshift = 11;
            st.gshift = 5;
            st.bshift = 0;
        }
        ARGB_1555 => {
            quantise_data =
                unsafe { histogram_16bit(&mut st, srcdata as *mut u16, width, height, srcformat) };
            st.alpha_data = true;
            hist_format = ARGB_1555;
            st.amul = 1;
            st.rmul = 31;
            st.gmul = 31;
            st.bmul = 31;
            st.ashift = 15;
            st.rshift = 10;
            st.gshift = 5;
            st.bshift = 0;
        }
        ARGB_4444 => {
            quantise_data =
                unsafe { histogram_16bit(&mut st, srcdata as *mut u16, width, height, srcformat) };
            st.alpha_data = true;
            hist_format = ARGB_4444;
            st.amul = 15;
            st.rmul = 15;
            st.gmul = 15;
            st.bmul = 15;
            st.ashift = 12;
            st.rshift = 8;
            st.gshift = 4;
            st.bshift = 0;
        }
        RGB_565 => {
            quantise_data =
                unsafe { histogram_16bit(&mut st, srcdata as *mut u16, width, height, srcformat) };
            hist_format = RGB_565;
            st.amul = 0;
            st.rmul = 31;
            st.gmul = 63;
            st.bmul = 31;
            st.ashift = 0;
            st.rshift = 11;
            st.gshift = 5;
            st.bshift = 0;
        }
        _ => return 0,
    }

    // Box 0 spans the full colour range.
    st.boxlist[0] = BoxT {
        amin: 0,
        amax: st.amul,
        rmin: 0,
        rmax: st.rmul,
        gmin: 0,
        gmax: st.gmul,
        bmin: 0,
        bmax: st.bmul,
        volume: 0,
        colour_count: st.colour_count,
    };
    st.boxlist[0].volume = if st.alpha_data {
        (st.amul as u32)
            .wrapping_mul(ASCALE)
            .wrapping_mul((st.rmul as u32).wrapping_mul(RSCALE))
            .wrapping_mul((st.gmul as u32).wrapping_mul(GSCALE))
            .wrapping_mul((st.bmul as u32).wrapping_mul(BSCALE)) as i32
    } else {
        (st.rmul as u32)
            .wrapping_mul(RSCALE)
            .wrapping_mul((st.gmul as u32).wrapping_mul(GSCALE))
            .wrapping_mul((st.bmul as u32).wrapping_mul(BSCALE)) as i32
    };

    if st.alpha_data {
        has_key = false;
    }
    if has_key {
        if nentries > 1 {
            nentries -= 1;
        } else {
            // Quirk: the quantise buffer is not freed on this path (C++ leak).
            return 0;
        }
    }

    let mut nboxes = 1i32;
    update_box_info(&mut st, 0);

    while nboxes < nentries {
        let found = if nboxes < nentries / 2 {
            find_most_populated_box(&st.boxlist, nboxes)
        } else {
            find_box_with_largest_volume(&st.boxlist, nboxes)
        };
        let Some(bi) = found else { break };

        st.boxlist[nboxes as usize] = st.boxlist[bi];
        let b = st.boxlist[bi];
        let lr = ((b.rmax - b.rmin) as u32).wrapping_mul(RSCALE) as i32;
        let lg = ((b.gmax - b.gmin) as u32).wrapping_mul(GSCALE) as i32;
        let lb = ((b.bmax - b.bmin) as u32).wrapping_mul(BSCALE) as i32;
        let (mut split, mut max);
        if st.alpha_data {
            let la = ((b.amax - b.amin) as u32).wrapping_mul(ASCALE) as i32;
            split = 0;
            max = la;
            if lg > max {
                max = lg;
                split = 2;
            }
        } else {
            split = 2;
            max = lg;
        }
        if lr > max {
            max = lr;
            split = 1;
        }
        if lb > max {
            split = 3;
        }

        let nb = nboxes as usize;
        match split {
            0 => {
                let s = (b.amax + b.amin) >> 1;
                st.boxlist[bi].amax = s;
                st.boxlist[nb].amin = s + 1;
            }
            1 => {
                let s = (b.rmax + b.rmin) >> 1;
                st.boxlist[bi].rmax = s;
                st.boxlist[nb].rmin = s + 1;
            }
            2 => {
                let s = (b.gmax + b.gmin) >> 1;
                st.boxlist[bi].gmax = s;
                st.boxlist[nb].gmin = s + 1;
            }
            _ => {
                let s = (b.bmax + b.bmin) >> 1;
                st.boxlist[bi].bmax = s;
                st.boxlist[nb].bmin = s + 1;
            }
        }
        update_box_info(&mut st, bi);
        update_box_info(&mut st, nb);
        nboxes += 1;
    }

    if has_key {
        nboxes += 1;
    }

    if st.alpha_data {
        compute_alpha_palette(&mut st, paldata, nboxes);
    } else {
        compute_palette(&mut st, paldata, nboxes, has_key);
    }

    unsafe {
        copy_palettised_data(
            &st,
            srcdata,
            quantise_data,
            dstdata,
            srcformat,
            width,
            height,
            has_key,
            keyval,
        );
    }

    if generate_mipmaps {
        let mip_dst = dstdata.wrapping_add((width as usize).wrapping_mul(height as usize));
        nboxes = unsafe {
            generate_mipmap(
                &mut st,
                dstdata,
                mip_dst,
                paldata,
                hist_format,
                width,
                height,
                has_key,
                nboxes,
                nentries,
            )
        };
    }

    unsafe { (st.svc.free)(quantise_data as *mut c_void) };
    nboxes
}
