//! Hand-written CoreText / CoreFoundation / CoreGraphics extern "C" bindings.
//!
//! These mirror exactly the subset of the Apple SDK surface that
//! `HarryPotter2/Unreal/NativeText/Src/NativeTextShared.cpp` used to call
//! directly. Struct layouts and enum values below were cross-checked against
//! the macOS SDK headers (`CTFontDescriptor.h`, `CGImage.h`) on this host;
//! see the doc comment on each constant for the exact header value it mirrors.
//!
//! Gated by the `coretext` feature, the same way `freetype_sys` gates its
//! `FT_*` declarations behind the `freetype` feature: bare `cargo
//! build`/`cargo test` need no native CoreText linkage, which only CMake's
//! final `hp2_game` executable link provides on macOS (via
//! `HP2_TEXT_PROVIDER_LINK_LIBS`). CoreText/CoreFoundation/CoreGraphics are
//! Apple system frameworks (not pinned/vendored like FreeType), so linking
//! them directly here is safe for any macOS toolchain.
use core::ffi::{c_char, c_void};

pub type CGFloat = f64;
pub type CFIndex = isize;
pub type CGGlyph = u16;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CGAffineTransform {
    pub a: CGFloat,
    pub b: CGFloat,
    pub c: CGFloat,
    pub d: CGFloat,
    pub tx: CGFloat,
    pub ty: CGFloat,
}

/// `CTFontDescriptor.h`: `kCTFontUIFontSystem = 2`.
pub const K_CT_FONT_UI_FONT_SYSTEM: u32 = 2;
/// `CTFontDescriptor.h`: `kCTFontUIFontEmphasizedSystem = 3`.
pub const K_CT_FONT_UI_FONT_EMPHASIZED_SYSTEM: u32 = 3;
/// `CTFontDescriptor.h`: `kCTFontOrientationDefault = 0`.
pub const K_CT_FONT_ORIENTATION_DEFAULT: u32 = 0;
/// `CFString.h`: `kCFStringEncodingUTF8 = 0x08000100`.
pub const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
/// `CFNumber.h`: `kCFNumberCGFloatType = 16`.
pub const K_CF_NUMBER_CG_FLOAT_TYPE: CFIndex = 16;
/// `CGImage.h`: `kCGImageAlphaPremultipliedLast = 1` (second `CGImageAlphaInfo` enumerator).
pub const K_CG_IMAGE_ALPHA_PREMULTIPLIED_LAST: u32 = 1;
/// `CGImage.h`: `kCGImageByteOrder32Big = (4 << 12)`, aliased by the deprecated
/// `kCGBitmapByteOrder32Big` the original C++ used.
pub const K_CG_BITMAP_BYTE_ORDER_32_BIG: u32 = 4 << 12;
/// `CGContext.h`: `kCGTextFill = 0` (first `CGTextDrawingMode` enumerator).
pub const K_CG_TEXT_FILL: i32 = 0;

#[cfg_attr(target_os = "macos", link(name = "CoreFoundation", kind = "framework"))]
unsafe extern "C" {
    pub static kCFAllocatorDefault: *const c_void;

    pub fn CFRelease(cf: *const c_void);
    pub fn CFRetain(cf: *const c_void) -> *const c_void;
    pub fn CFGetTypeID(cf: *const c_void) -> usize;
    pub fn CFStringGetTypeID() -> usize;
    pub fn CFStringCreateWithCharacters(
        alloc: *const c_void,
        chars: *const u16,
        num_chars: CFIndex,
    ) -> *const c_void;
    pub fn CFStringGetLength(the_string: *const c_void) -> CFIndex;
    pub fn CFStringGetMaximumSizeForEncoding(length: CFIndex, encoding: u32) -> CFIndex;
    pub fn CFStringGetCString(
        the_string: *const c_void,
        buffer: *mut c_char,
        buffer_size: CFIndex,
        encoding: u32,
    ) -> u8;
    pub fn CFStringGetRangeOfComposedCharactersAtIndex(
        the_string: *const c_void,
        the_index: CFIndex,
    ) -> CFRange;
    pub fn CFArrayGetCount(the_array: *const c_void) -> CFIndex;
    pub fn CFArrayGetValueAtIndex(the_array: *const c_void, idx: CFIndex) -> *const c_void;
    pub fn CFDictionaryGetValue(the_dict: *const c_void, key: *const c_void) -> *const c_void;
    pub fn CFAttributedStringCreateMutable(
        alloc: *const c_void,
        max_length: CFIndex,
    ) -> *const c_void;
    pub fn CFAttributedStringReplaceString(
        a_str: *const c_void,
        range: CFRange,
        replacement: *const c_void,
    );
    pub fn CFAttributedStringSetAttribute(
        a_str: *const c_void,
        range: CFRange,
        attr_name: *const c_void,
        value: *const c_void,
    );
    pub fn CFNumberCreate(
        allocator: *const c_void,
        the_type: CFIndex,
        value_ptr: *const c_void,
    ) -> *const c_void;
}

#[cfg_attr(target_os = "macos", link(name = "CoreText", kind = "framework"))]
unsafe extern "C" {
    pub static kCTFontNameAttribute: *const c_void;
    pub static kCTFontAttributeName: *const c_void;
    pub static kCTKernAttributeName: *const c_void;

    pub fn CTFontManagerCopyAvailableFontFamilyNames() -> *const c_void;
    pub fn CTFontCreateWithName(
        name: *const c_void,
        size: CGFloat,
        matrix: *const CGAffineTransform,
    ) -> *const c_void;
    pub fn CTFontCreateUIFontForLanguage(
        ui_type: u32,
        size: CGFloat,
        language: *const c_void,
    ) -> *const c_void;
    pub fn CTFontCopyFontDescriptor(font: *const c_void) -> *const c_void;
    pub fn CTFontDescriptorCopyAttribute(
        descriptor: *const c_void,
        attribute: *const c_void,
    ) -> *const c_void;
    pub fn CTFontCopyPostScriptName(font: *const c_void) -> *const c_void;
    pub fn CTFontGetMatrix(font: *const c_void) -> CGAffineTransform;
    pub fn CTFontGetSize(font: *const c_void) -> CGFloat;
    pub fn CTFontGetAscent(font: *const c_void) -> CGFloat;
    pub fn CTFontGetDescent(font: *const c_void) -> CGFloat;
    pub fn CTFontGetLeading(font: *const c_void) -> CGFloat;
    pub fn CTFontGetUnderlinePosition(font: *const c_void) -> CGFloat;
    pub fn CTFontGetUnderlineThickness(font: *const c_void) -> CGFloat;
    pub fn CTFontGetBoundingRectsForGlyphs(
        font: *const c_void,
        orientation: u32,
        glyphs: *const CGGlyph,
        bounding_rects: *mut CGRect,
        count: CFIndex,
    ) -> CGRect;
    pub fn CTFontDrawGlyphs(
        font: *const c_void,
        glyphs: *const CGGlyph,
        positions: *const CGPoint,
        count: usize,
        context: *const c_void,
    );
    pub fn CTTypesetterCreateWithAttributedString(string: *const c_void) -> *const c_void;
    pub fn CTTypesetterSuggestClusterBreak(
        typesetter: *const c_void,
        start_index: CFIndex,
        width: f64,
    ) -> CFIndex;
    pub fn CTTypesetterSuggestLineBreak(
        typesetter: *const c_void,
        start_index: CFIndex,
        width: f64,
    ) -> CFIndex;
    pub fn CTTypesetterCreateLine(
        typesetter: *const c_void,
        string_range: CFRange,
    ) -> *const c_void;
    pub fn CTLineGetTypographicBounds(
        line: *const c_void,
        ascent: *mut CGFloat,
        descent: *mut CGFloat,
        leading: *mut CGFloat,
    ) -> f64;
    pub fn CTLineGetGlyphRuns(line: *const c_void) -> *const c_void;
    pub fn CTLineGetOffsetForStringIndex(
        line: *const c_void,
        char_index: CFIndex,
        secondary_offset: *mut CGFloat,
    ) -> f64;
    pub fn CTRunGetStringRange(run: *const c_void) -> CFRange;
    pub fn CTRunGetAttributes(run: *const c_void) -> *const c_void;
    pub fn CTRunGetGlyphCount(run: *const c_void) -> CFIndex;
    pub fn CTRunGetStringIndices(run: *const c_void, range: CFRange, buffer: *mut CFIndex);
    pub fn CTRunGetGlyphs(run: *const c_void, range: CFRange, buffer: *mut CGGlyph);
    pub fn CTRunGetPositions(run: *const c_void, range: CFRange, buffer: *mut CGPoint);
    pub fn CTRunGetAdvances(run: *const c_void, range: CFRange, buffer: *mut CGSize);
}

#[cfg_attr(target_os = "macos", link(name = "CoreGraphics", kind = "framework"))]
unsafe extern "C" {
    pub fn CGRectIsNull(rect: CGRect) -> u8;
    pub fn CGColorSpaceCreateDeviceRGB() -> *const c_void;
    pub fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: *const c_void,
        bitmap_info: u32,
    ) -> *const c_void;
    pub fn CGContextSetShouldAntialias(c: *const c_void, should_antialias: u8);
    pub fn CGContextSetShouldSmoothFonts(c: *const c_void, should_smooth_fonts: u8);
    pub fn CGContextSetShouldSubpixelPositionFonts(c: *const c_void, should_subpixel_position: u8);
    pub fn CGContextSetShouldSubpixelQuantizeFonts(c: *const c_void, should_subpixel_quantize: u8);
    pub fn CGContextSetTextDrawingMode(c: *const c_void, mode: i32);
    pub fn CGContextSetRGBFillColor(
        c: *const c_void,
        red: CGFloat,
        green: CGFloat,
        blue: CGFloat,
        alpha: CGFloat,
    );
    pub fn CGContextTranslateCTM(c: *const c_void, tx: CGFloat, ty: CGFloat);
}
