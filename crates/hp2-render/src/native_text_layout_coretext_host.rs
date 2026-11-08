//! Rust-native `Hp2TextLayoutCoreTextServices` implementation.
//!
//! `native_text_layout_coretext.rs`'s policy code (font selection, line
//! breaking, cluster/run bookkeeping, raster rectangle math) is unchanged and
//! still calls through the `Hp2TextLayoutCoreTextServices` function-pointer
//! table exactly as before. [`services`] returns a table of 20 entries that
//! all call CoreText / CoreFoundation / CoreGraphics directly (via
//! `coretext_sys`), matching the FreeType provider's `hp2_rs_ft_*` exports
//! owning SDK object lifetimes directly in Rust. `FCanvasTextLayout` itself
//! is fully Rust-owned (see `native_text_layout_coretext::Layout`); none of
//! these leaves forward into C++ or can throw (CoreText/CoreFoundation SDK
//! calls report failure via `null`/`0` returns, never C++ exceptions, and
//! the `Vec` scratch buffers abort on allocation failure like the rest of
//! this codebase rather than unwinding).
use crate::coretext_sys as ct;
use crate::native_text_layout::Hp2TextLayoutRange;
use crate::native_text_layout_coretext::{
    Hp2TextLayoutCoreTextFontInfo, Hp2TextLayoutCoreTextGlyph, Hp2TextLayoutCoreTextMetrics,
    Hp2TextLayoutCoreTextRun, Hp2TextLayoutCoreTextServices,
};
use core::ffi::c_void;

/// Per-call scratch state threaded through every `Hp2TextLayoutCoreTextServices`
/// entry as the opaque `ctx`. Replaces the native-call fields that used to
/// live on C++'s `FContext` (`PostScript`, `Descriptor`, `Glyphs`, `Positions`,
/// `Advances`, `Indices`, `Bounds`, `Run`); every field below is Rust-owned
/// scratch storage, re-used across calls purely to avoid per-call
/// allocation.
pub struct Hp2TextLayoutCoreTextScratch {
    postscript: Vec<u8>,
    descriptor: Vec<u8>,
    glyphs: Vec<ct::CGGlyph>,
    positions: Vec<ct::CGPoint>,
    advances: Vec<ct::CGSize>,
    indices: Vec<ct::CFIndex>,
    bounds: Vec<ct::CGRect>,
    run: Vec<Hp2TextLayoutCoreTextGlyph>,
}
impl Hp2TextLayoutCoreTextScratch {
    pub fn new() -> Self {
        Self {
            postscript: Vec::new(),
            descriptor: Vec::new(),
            glyphs: Vec::new(),
            positions: Vec::new(),
            advances: Vec::new(),
            indices: Vec::new(),
            bounds: Vec::new(),
            run: Vec::new(),
        }
    }
}
impl Default for Hp2TextLayoutCoreTextScratch {
    fn default() -> Self {
        Self::new()
    }
}

unsafe fn scratch<'a>(ctx: *mut c_void) -> &'a mut Hp2TextLayoutCoreTextScratch {
    unsafe { &mut *ctx.cast::<Hp2TextLayoutCoreTextScratch>() }
}

/// Mirrors the original C++ `UTF8(CFStringRef)`: converts via
/// `CFStringGetCString` and truncates at the embedded NUL, exactly like
/// `std::string(Bytes.data())` did.
fn utf8(string: *const c_void) -> Vec<u8> {
    if string.is_null() {
        return Vec::new();
    }
    let length = unsafe { ct::CFStringGetLength(string) };
    if length == 0 {
        return Vec::new();
    }
    let capacity =
        unsafe { ct::CFStringGetMaximumSizeForEncoding(length, ct::K_CF_STRING_ENCODING_UTF8) } + 1;
    if capacity <= 0 {
        return Vec::new();
    }
    let mut bytes = vec![0u8; capacity as usize];
    let ok = unsafe {
        ct::CFStringGetCString(
            string,
            bytes.as_mut_ptr().cast(),
            capacity,
            ct::K_CF_STRING_ENCODING_UTF8,
        )
    };
    if ok == 0 {
        return Vec::new();
    }
    let nul = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes.truncate(nul);
    bytes
}

unsafe extern "C" fn release_leaf(handle: *const c_void) {
    unsafe { ct::CFRelease(handle) };
}
unsafe extern "C" fn retain_leaf(handle: *const c_void) {
    unsafe { ct::CFRetain(handle) };
}
unsafe extern "C" fn named_font_leaf(
    _ctx: *mut c_void,
    name: *const u16,
    length: usize,
    size: f64,
) -> *const c_void {
    let string =
        unsafe { ct::CFStringCreateWithCharacters(ct::kCFAllocatorDefault, name, length as isize) };
    if string.is_null() {
        return core::ptr::null();
    }
    let font = unsafe { ct::CTFontCreateWithName(string, size, core::ptr::null()) };
    unsafe { ct::CFRelease(string) };
    font
}
unsafe extern "C" fn system_font_leaf(_ctx: *mut c_void, bold: u8, size: f64) -> *const c_void {
    let ui_type = if bold != 0 {
        ct::K_CT_FONT_UI_FONT_EMPHASIZED_SYSTEM
    } else {
        ct::K_CT_FONT_UI_FONT_SYSTEM
    };
    unsafe { ct::CTFontCreateUIFontForLanguage(ui_type, size, core::ptr::null()) }
}
unsafe extern "C" fn font_info_leaf(
    ctx: *mut c_void,
    handle: *const c_void,
    out: *mut Hp2TextLayoutCoreTextFontInfo,
) -> u8 {
    let scratch = unsafe { scratch(ctx) };
    let descriptor = unsafe { ct::CTFontCopyFontDescriptor(handle) };
    let name = if !descriptor.is_null() {
        unsafe { ct::CTFontDescriptorCopyAttribute(descriptor, ct::kCTFontNameAttribute) }
    } else {
        core::ptr::null()
    };
    let postscript = unsafe { ct::CTFontCopyPostScriptName(handle) };
    scratch.postscript = utf8(postscript);
    scratch.descriptor = if !name.is_null()
        && unsafe { ct::CFGetTypeID(name) } == unsafe { ct::CFStringGetTypeID() }
    {
        utf8(name)
    } else {
        Vec::new()
    };
    let matrix = unsafe { ct::CTFontGetMatrix(handle) };
    let size = unsafe { ct::CTFontGetSize(handle) };
    if !descriptor.is_null() {
        unsafe { ct::CFRelease(descriptor) };
    }
    if !name.is_null() {
        unsafe { ct::CFRelease(name) };
    }
    if !postscript.is_null() {
        unsafe { ct::CFRelease(postscript) };
    }
    unsafe {
        *out = Hp2TextLayoutCoreTextFontInfo {
            postscript: scratch.postscript.as_ptr(),
            postscript_len: scratch.postscript.len(),
            descriptor: scratch.descriptor.as_ptr(),
            descriptor_len: scratch.descriptor.len(),
            size,
            matrix: [matrix.a, matrix.b, matrix.c, matrix.d, matrix.tx, matrix.ty],
        };
    }
    1
}
unsafe extern "C" fn source_leaf(
    _ctx: *mut c_void,
    text: *const u16,
    length: usize,
) -> *const c_void {
    unsafe { ct::CFStringCreateWithCharacters(ct::kCFAllocatorDefault, text, length as isize) }
}
unsafe extern "C" fn attributes_leaf(_ctx: *mut c_void, source: *const c_void) -> *const c_void {
    let owner = unsafe { ct::CFAttributedStringCreateMutable(ct::kCFAllocatorDefault, 0) };
    if owner.is_null() {
        return core::ptr::null();
    }
    unsafe {
        ct::CFAttributedStringReplaceString(
            owner,
            ct::CFRange {
                location: 0,
                length: 0,
            },
            source,
        )
    };
    owner
}
unsafe extern "C" fn number_leaf(_ctx: *mut c_void, value: f64) -> *const c_void {
    let value: ct::CGFloat = value;
    unsafe {
        ct::CFNumberCreate(
            ct::kCFAllocatorDefault,
            ct::K_CF_NUMBER_CG_FLOAT_TYPE,
            (&raw const value).cast(),
        )
    }
}
unsafe extern "C" fn set_font_leaf(
    _ctx: *mut c_void,
    attributes: *const c_void,
    font: *const c_void,
    length: usize,
) -> u8 {
    unsafe {
        ct::CFAttributedStringSetAttribute(
            attributes,
            ct::CFRange {
                location: 0,
                length: length as isize,
            },
            ct::kCTFontAttributeName,
            font,
        )
    };
    1
}
unsafe extern "C" fn set_kern_leaf(
    _ctx: *mut c_void,
    attributes: *const c_void,
    number: *const c_void,
    length: usize,
) -> u8 {
    unsafe {
        ct::CFAttributedStringSetAttribute(
            attributes,
            ct::CFRange {
                location: 0,
                length: length as isize,
            },
            ct::kCTKernAttributeName,
            number,
        )
    };
    1
}
unsafe extern "C" fn typesetter_leaf(
    _ctx: *mut c_void,
    attributes: *const c_void,
) -> *const c_void {
    unsafe { ct::CTTypesetterCreateWithAttributedString(attributes) }
}
unsafe extern "C" fn suggest_leaf(
    _ctx: *mut c_void,
    typesetter: *const c_void,
    start: isize,
    width: f64,
    cluster: u8,
) -> isize {
    if cluster != 0 {
        unsafe { ct::CTTypesetterSuggestClusterBreak(typesetter, start, width) }
    } else {
        unsafe { ct::CTTypesetterSuggestLineBreak(typesetter, start, width) }
    }
}
unsafe extern "C" fn line_leaf(
    _ctx: *mut c_void,
    typesetter: *const c_void,
    range: Hp2TextLayoutRange,
) -> *const c_void {
    unsafe {
        ct::CTTypesetterCreateLine(
            typesetter,
            ct::CFRange {
                location: range.location,
                length: range.length,
            },
        )
    }
}
unsafe extern "C" fn metrics_leaf(
    _ctx: *mut c_void,
    handle: *const c_void,
    is_font: u8,
    out: *mut Hp2TextLayoutCoreTextMetrics,
) -> u8 {
    let mut value = Hp2TextLayoutCoreTextMetrics::default();
    if is_font != 0 {
        value.ascent = unsafe { ct::CTFontGetAscent(handle) };
        value.descent = unsafe { ct::CTFontGetDescent(handle) };
        value.leading = unsafe { ct::CTFontGetLeading(handle) };
        value.underline = unsafe { ct::CTFontGetUnderlinePosition(handle) };
        value.thickness = unsafe { ct::CTFontGetUnderlineThickness(handle) };
    } else {
        value.advance = unsafe {
            ct::CTLineGetTypographicBounds(
                handle,
                &mut value.ascent,
                &mut value.descent,
                &mut value.leading,
            )
        };
    }
    unsafe { *out = value };
    1
}
unsafe extern "C" fn component_leaf(
    _ctx: *mut c_void,
    source: *const c_void,
    index: isize,
) -> Hp2TextLayoutRange {
    let range = unsafe { ct::CFStringGetRangeOfComposedCharactersAtIndex(source, index) };
    Hp2TextLayoutRange {
        location: range.location,
        length: range.length,
    }
}
unsafe extern "C" fn run_count_leaf(_ctx: *mut c_void, line: *const c_void) -> usize {
    let runs = unsafe { ct::CTLineGetGlyphRuns(line) };
    if runs.is_null() {
        0
    } else {
        unsafe { ct::CFArrayGetCount(runs) as usize }
    }
}
unsafe extern "C" fn run_leaf(
    ctx: *mut c_void,
    line: *const c_void,
    index: usize,
    capture: u8,
    out: *mut Hp2TextLayoutCoreTextRun,
) -> u8 {
    let scratch = unsafe { scratch(ctx) };
    let run = unsafe { ct::CFArrayGetValueAtIndex(ct::CTLineGetGlyphRuns(line), index as isize) };
    let range = unsafe { ct::CTRunGetStringRange(run) };
    let attributes = unsafe { ct::CTRunGetAttributes(run) };
    let font = if !attributes.is_null() {
        unsafe { ct::CFDictionaryGetValue(attributes, ct::kCTFontAttributeName) }
    } else {
        core::ptr::null()
    };
    let count = unsafe { ct::CTRunGetGlyphCount(run) as usize };
    scratch.indices.resize(count, 0);
    scratch.run.resize(
        count,
        Hp2TextLayoutCoreTextGlyph {
            glyph: 0,
            position: [0.0; 2],
            advance: [0.0; 2],
            bounds: [0.0; 4],
            source: 0,
            null_bounds: 0,
        },
    );
    let whole = ct::CFRange {
        location: 0,
        length: 0,
    };
    if count != 0 {
        unsafe { ct::CTRunGetStringIndices(run, whole, scratch.indices.as_mut_ptr()) };
    }
    let capture = capture != 0;
    if capture && count != 0 {
        scratch.glyphs.resize(count, 0);
        scratch
            .positions
            .resize(count, ct::CGPoint { x: 0.0, y: 0.0 });
        scratch.advances.resize(
            count,
            ct::CGSize {
                width: 0.0,
                height: 0.0,
            },
        );
        scratch.bounds.resize(
            count,
            ct::CGRect {
                origin: ct::CGPoint { x: 0.0, y: 0.0 },
                size: ct::CGSize {
                    width: 0.0,
                    height: 0.0,
                },
            },
        );
        unsafe { ct::CTRunGetGlyphs(run, whole, scratch.glyphs.as_mut_ptr()) };
        unsafe { ct::CTRunGetPositions(run, whole, scratch.positions.as_mut_ptr()) };
        unsafe { ct::CTRunGetAdvances(run, whole, scratch.advances.as_mut_ptr()) };
        if !font.is_null() {
            unsafe {
                ct::CTFontGetBoundingRectsForGlyphs(
                    font,
                    ct::K_CT_FONT_ORIENTATION_DEFAULT,
                    scratch.glyphs.as_ptr(),
                    scratch.bounds.as_mut_ptr(),
                    count as isize,
                )
            };
        }
    }
    for i in 0..count {
        let g = &mut scratch.run[i];
        *g = Hp2TextLayoutCoreTextGlyph {
            glyph: 0,
            position: [0.0; 2],
            advance: [0.0; 2],
            bounds: [0.0; 4],
            source: scratch.indices[i],
            null_bounds: 0,
        };
        if capture {
            let b = scratch.bounds[i];
            g.glyph = scratch.glyphs[i];
            g.position = [scratch.positions[i].x, scratch.positions[i].y];
            g.advance = [scratch.advances[i].width, scratch.advances[i].height];
            g.bounds = [b.origin.x, b.origin.y, b.size.width, b.size.height];
            g.null_bounds = unsafe { ct::CGRectIsNull(b) };
        }
    }
    unsafe {
        *out = Hp2TextLayoutCoreTextRun {
            range: Hp2TextLayoutRange {
                location: range.location,
                length: range.length,
            },
            font,
            glyphs: scratch.run.as_ptr(),
            count,
        };
    }
    1
}
unsafe extern "C" fn offset_leaf(_ctx: *mut c_void, line: *const c_void, index: isize) -> f64 {
    unsafe { ct::CTLineGetOffsetForStringIndex(line, index, core::ptr::null_mut()) }
}
unsafe extern "C" fn raster_leaf(
    _ctx: *mut c_void,
    font: *const c_void,
    glyph: u16,
    width: i32,
    height: i32,
    min_x: f64,
    min_y: f64,
    antialias: u8,
    smooth: u8,
    subpixel: u8,
    quantize: u8,
    pixels: *mut u8,
) -> u8 {
    let color = unsafe { ct::CGColorSpaceCreateDeviceRGB() };
    if color.is_null() {
        return 0;
    }
    let bitmap = unsafe {
        ct::CGBitmapContextCreate(
            pixels.cast(),
            width as usize,
            height as usize,
            8,
            (width as usize) * 4,
            color,
            ct::K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST | ct::K_CG_BITMAP_BYTE_ORDER_32_BIG,
        )
    };
    unsafe { ct::CFRelease(color) };
    if bitmap.is_null() {
        return 0;
    }
    unsafe {
        ct::CGContextSetShouldAntialias(bitmap, antialias);
        ct::CGContextSetShouldSmoothFonts(bitmap, smooth);
        ct::CGContextSetShouldSubpixelPositionFonts(bitmap, subpixel);
        ct::CGContextSetShouldSubpixelQuantizeFonts(bitmap, quantize);
        ct::CGContextSetTextDrawingMode(bitmap, ct::K_CG_TEXT_FILL);
        ct::CGContextSetRGBFillColor(bitmap, 1.0, 1.0, 1.0, 1.0);
        ct::CGContextTranslateCTM(bitmap, -min_x, -min_y);
        let origin = ct::CGPoint { x: 0.0, y: 0.0 };
        ct::CTFontDrawGlyphs(font, &glyph, &origin, 1, bitmap);
        ct::CFRelease(bitmap);
    }
    1
}

/// Builds a fresh `Hp2TextLayoutCoreTextServices` table of Rust-native
/// function pointers (20 entries). Cheap: every field is a plain function
/// pointer.
pub const fn services() -> Hp2TextLayoutCoreTextServices {
    Hp2TextLayoutCoreTextServices {
        release: release_leaf,
        retain: retain_leaf,
        named_font: named_font_leaf,
        system_font: system_font_leaf,
        font_info: font_info_leaf,
        source: source_leaf,
        attributes: attributes_leaf,
        number: number_leaf,
        set_font: set_font_leaf,
        set_kern: set_kern_leaf,
        typesetter: typesetter_leaf,
        suggest: suggest_leaf,
        line: line_leaf,
        metrics: metrics_leaf,
        component: component_leaf,
        run_count: run_count_leaf,
        run: run_leaf,
        offset: offset_leaf,
        raster: raster_leaf,
    }
}

/// Rust port of `NativeTextShared.cpp`'s `Hp2NativeText::ProbeAvailability`:
/// same three checks in the same order (font catalog non-empty, a
/// representative role font resolves via [`crate::native_text_layout_coretext::role_font`],
/// the rasterizer can allocate a 1x1 ARGB bitmap), same three outcomes.
/// Returns `0` = ready, `1` = fonts_unavailable, `2` =
/// rasterizer_unavailable; the C++ caller maps the code back onto
/// `Engine/Inc/UnRenDev.h`'s pinned `"text.*"` reason-code strings so those
/// strings stay defined in exactly one place.
pub unsafe fn probe_availability(api: &Hp2TextLayoutCoreTextServices, ctx: *mut c_void) -> i32 {
    let families = unsafe { ct::CTFontManagerCopyAvailableFontFamilyNames() };
    if families.is_null() {
        return 1;
    }
    let count = unsafe { ct::CFArrayGetCount(families) };
    unsafe { ct::CFRelease(families) };
    if count == 0 {
        return 1;
    }
    // NTROLE_Body (`Engine/Inc/UnRenDev.h`'s `ENativeTextRole`), 12pt: the same
    // role/size the original C++ probed with.
    let font = unsafe { crate::native_text_layout_coretext::role_font(api, ctx, 2, 12.0) };
    if font.is_null() {
        return 1;
    }
    unsafe { (api.release)(font) };
    let color = unsafe { ct::CGColorSpaceCreateDeviceRGB() };
    if color.is_null() {
        return 2;
    }
    let mut pixel = [0u8; 4];
    let bitmap = unsafe {
        ct::CGBitmapContextCreate(
            pixel.as_mut_ptr().cast(),
            1,
            1,
            8,
            4,
            color,
            ct::K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST | ct::K_CG_BITMAP_BYTE_ORDER_32_BIG,
        )
    };
    unsafe { ct::CFRelease(color) };
    if bitmap.is_null() {
        return 2;
    }
    unsafe { ct::CFRelease(bitmap) };
    0
}

#[cfg(test)]
#[allow(non_snake_case)]
mod coretext_oracle_tests {
    //! End-to-end oracle: exercises the Rust-native leaves above against the
    //! real CoreText/CoreFoundation/CoreGraphics frameworks through the
    //! exact same `create()`/`raster()` policy code the production FFI
    //! boundary calls, inspecting the opaque `Layout` through its public
    //! `view()` accessor (glyph ids/positions/advances/bounds/identity keys)
    //! and the rasterized alpha bytes `raster()` returns.
    use super::*;
    use crate::native_text_layout_coretext::Hp2TextLayoutCoreTextRequest;

    extern "C" fn no_pages(_font: *const c_void, _page: i32) -> i32 {
        0
    }

    fn request(text: &[u16], family: &[u16]) -> Hp2TextLayoutCoreTextRequest {
        Hp2TextLayoutCoreTextRequest {
            text: text.as_ptr().cast(),
            length: text.len() as i32,
            unit_bytes: 2,
            name: core::ptr::null(),
            name_length: 0,
            family: family.as_ptr().cast(),
            family_length: family.len(),
            // Non-null only to pass `create()`'s guard check: `family_length`
            // is non-zero here, so the legacy bitmap page scan never runs.
            font: text.as_ptr().cast(),
            font_height: 24,
            page_count: 0,
            scale: 1.0,
            space_x: 0.0,
            space_y: 0.0,
            clip_x: 1000.0,
            clip_y: 1000.0,
            color: [1.0, 1.0, 1.0, 1.0],
            start_x: 0,
            start_y: 0,
            visible: 0,
            ampersand: 0,
            center: 0,
            wrapped: 0,
            poly_flags: 0,
            clip: 0,
        }
    }

    #[test]
    fn create_measure_and_raster_round_trip_through_real_coretext() {
        let text: Vec<u16> = "Hi".encode_utf16().collect();
        let family: Vec<u16> = "Helvetica".encode_utf16().collect();
        let view = request(&text, &family);
        let mut scratch = Hp2TextLayoutCoreTextScratch::new();
        let layout = unsafe {
            super::super::native_text_layout_coretext::create(
                &SERVICES_FOR_TEST,
                (&mut scratch as *mut Hp2TextLayoutCoreTextScratch).cast(),
                &view,
                no_pages,
            )
        }
        .expect("real CoreText system fonts must be present on this host");
        let output = layout.view();
        assert!(
            output.height > 0,
            "non-empty text must occupy at least one line"
        );
        assert!(
            output.ascent > 0.0 && output.descent > 0.0,
            "real font metrics must be positive"
        );
        assert_eq!(
            output.glyph_count, 2,
            "\"Hi\" must shape to exactly two glyphs"
        );
        let glyphs = unsafe { core::slice::from_raw_parts(output.glyphs, output.glyph_count) };
        assert!(
            glyphs[0].advance[0] > 0.0,
            "a visible glyph must have a positive horizontal advance"
        );
        assert!(
            glyphs.iter().all(|g| g.identity_len > 0),
            "every drawable glyph must carry a non-empty font identity key"
        );
        assert!(
            glyphs[1].position[0] > glyphs[0].position[0],
            "second glyph must advance to the right of the first"
        );
        let drawable = glyphs
            .iter()
            .find(|g| g.drawable != 0)
            .expect("at least one glyph in \"Hi\" must be drawable");
        let (width, height, alpha) = unsafe {
            super::super::native_text_layout_coretext::raster(
                &SERVICES_FOR_TEST,
                core::ptr::null_mut(),
                drawable.font,
                drawable.glyph,
                drawable.bounds,
                true,
                1024,
            )
        }
        .expect("rasterizing a real system-font glyph must succeed");
        assert!(width > 0 && height > 0);
        assert_eq!(alpha.len(), (width * height) as usize);
        assert!(
            alpha.iter().any(|&a| a != 0),
            "drawing a visible glyph must paint at least one non-transparent pixel"
        );
    }

    #[test]
    fn probe_availability_reports_ready_on_real_coretext() {
        let mut scratch = Hp2TextLayoutCoreTextScratch::new();
        let code = unsafe {
            super::probe_availability(
                &SERVICES_FOR_TEST,
                (&mut scratch as *mut Hp2TextLayoutCoreTextScratch).cast(),
            )
        };
        assert_eq!(
            code, 0,
            "a real macOS CoreText stack with system fonts installed must report ready (0), not fonts_unavailable (1) or rasterizer_unavailable (2)"
        );
    }

    static SERVICES_FOR_TEST: Hp2TextLayoutCoreTextServices = services();
}
