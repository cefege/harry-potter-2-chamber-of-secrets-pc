//! Rust-native FreeType, HarfBuzz, and Fontconfig services for native text.
use crate::fontconfig_sys as fc;
use crate::freetype_sys as ft;
use crate::harfbuzz_sys as hb;
use crate::native_text_layout_freetype::{
    Hp2TextLayoutFreeTypeBitmap, Hp2TextLayoutFreeTypeGlyph, Hp2TextLayoutFreeTypeMetrics,
    Hp2TextLayoutFreeTypeServices,
};
use core::ffi::{c_char, c_long, c_void, CStr};
use core::sync::atomic::{AtomicPtr, Ordering};

static LIBRARY: AtomicPtr<c_void> = AtomicPtr::new(core::ptr::null_mut());

/// A page's raw serialized glyph records, as read by native-text font sizing.
#[derive(Debug)]
pub struct Hp2TextLayoutFreeTypePage {
    data: Vec<u8>,
    count: usize,
    stride: usize,
    offset: usize,
}

impl Hp2TextLayoutFreeTypePage {
    /// Creates an owned page buffer and describes its serialized glyph records.
    pub fn new(data: Vec<u8>, count: usize, stride: usize, offset: usize) -> Self {
        Self { data, count, stride, offset }
    }
}

/// Per-render-owner state backing native FreeType service callbacks.
///
/// The callback state has a stable allocation when this owner moves. Keep the
/// owner alive while using its service table, and serialize calls through any
/// one table; provider calls are serialized by the layout engine.
#[derive(Debug, Default)]
pub struct Hp2TextLayoutFreeTypeHost {
    state: Box<HostState>,
}

#[derive(Debug, Default)]
struct HostState {
    pages: Vec<Hp2TextLayoutFreeTypePage>,
    alpha: Vec<u8>,
    shaped: Vec<Hp2TextLayoutFreeTypeGlyph>,
    resolved_path: Vec<u8>,
}

impl Hp2TextLayoutFreeTypeHost {
    /// Creates an empty native-text owner.
    pub fn new() -> Self { Self::default() }

    /// Adds an owned engine font page used when selecting a default font size.
    ///
    /// `count`, `stride`, and `offset` describe records containing an unaligned
    /// `i32` font height at `offset` bytes into each record.
    pub fn add_page(&mut self, page: Hp2TextLayoutFreeTypePage) { self.state.pages.push(page); }

    /// Transfers the most recently rasterized alpha bitmap to the caller.
    ///
    /// The result is empty until rasterization has written a bitmap. Call this
    /// after each raster request whose pixels must be retained.
    pub fn take_alpha_buffer(&mut self) -> Vec<u8> { core::mem::take(&mut self.state.alpha) }

    /// Returns the real SDK service table bound to this owner.
    pub fn services(&mut self) -> Hp2TextLayoutFreeTypeServices {
        Hp2TextLayoutFreeTypeServices {
            context: (&mut *self.state as *mut HostState).cast(), init, resolve, open, close, coverage, shape,
            render, page, alpha_buffer,
        }
    }
}

unsafe fn host<'a>(ctx: *mut c_void) -> &'a mut HostState {
    // SAFETY: the service contract requires context to point to the live owner's state.
    unsafe { &mut *ctx.cast() }
}

unsafe extern "C" fn init(_ctx: *mut c_void) -> i32 {
    // SAFETY: FreeType accepts a writable library output and owns it until teardown.
    unsafe {
        if !LIBRARY.load(Ordering::Acquire).is_null() { return 1; }
        let mut library = core::ptr::null_mut();
        if ft::FT_Init_FreeType(&mut library) != 0 || library.is_null() { return 0; }
        match LIBRARY.compare_exchange(core::ptr::null_mut(), library, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => 1,
            Err(_) => { ft::FT_Done_FreeType(library); 1 }
        }
    }
}

unsafe extern "C" fn open(_ctx: *mut c_void, path: *const u8, length: usize, index: i32, size64: u32, out_face: *mut usize, out_metrics: *mut Hp2TextLayoutFreeTypeMetrics) -> i32 {
    if path.is_null() || out_face.is_null() || out_metrics.is_null() { return 0; }
    let library = LIBRARY.load(Ordering::Acquire);
    if library.is_null() { return 0; }
    let Ok(path) = std::ffi::CString::new(unsafe { core::slice::from_raw_parts(path, length) }) else { return 0; };
    // SAFETY: arguments are valid and the library was initialized above.
    unsafe {
        let mut face: *mut ft::FtFaceRecPrefix = core::ptr::null_mut();
        if ft::FT_New_Face(library, path.as_ptr(), index as c_long, &mut face) != 0 || face.is_null() { return 0; }
        if ft::FT_Set_Char_Size(face, 0, size64 as c_long, 72, 72) != 0 { ft::FT_Done_Face(face); return 0; }
        let size = (*face).size;
        if size.is_null() { ft::FT_Done_Face(face); return 0; }
        let metrics = (*size).metrics;
        *out_metrics = Hp2TextLayoutFreeTypeMetrics {
            ascent: metrics.ascender as f64 / 64.0,
            descent: -(metrics.descender as f64) / 64.0,
            height: metrics.height as f64 / 64.0,
            underline_position: ft::FT_MulFix((*face).underline_position as _, metrics.y_scale) as f64 / 64.0,
            underline_thickness: ft::FT_MulFix((*face).underline_thickness as _, metrics.y_scale) as f64 / 64.0,
        };
        *out_face = face as usize;
        1
    }
}

unsafe extern "C" fn close(_ctx: *mut c_void, face: usize) -> i32 {
    if face == 0 { return 0; }
    // SAFETY: provider invokes close exactly once for a live face returned by open.
    unsafe { ft::FT_Done_Face(face as *mut ft::FtFaceRecPrefix) };
    1
}

unsafe extern "C" fn coverage(_ctx: *mut c_void, face: usize, scalars: *const u32, count: usize, out: *mut u32) -> i32 {
    if face == 0 || scalars.is_null() || out.is_null() { return 0; }
    // SAFETY: pointers and count are guaranteed by the services callback contract.
    unsafe { for i in 0..count { *out.add(i) = ft::FT_Get_Char_Index(face as *mut ft::FtFaceRecPrefix, *scalars.add(i) as _) } }
    1
}

unsafe extern "C" fn render(_ctx: *mut c_void, face: usize, glyphs: *const u32, count: usize, out: *mut Hp2TextLayoutFreeTypeBitmap) -> i32 {
    if face == 0 || glyphs.is_null() || out.is_null() { return 0; }
    // SAFETY: the face is live and the input/output arrays cover count entries.
    unsafe {
        let face = face as *mut ft::FtFaceRecPrefix;
        for i in 0..count {
            if ft::FT_Load_Glyph(face, *glyphs.add(i), ft::FT_LOAD_DEFAULT | ft::FT_LOAD_RENDER) != 0 { return 0; }
            let slot = (*face).glyph;
            let bitmap = (*slot).bitmap;
            *out.add(i) = Hp2TextLayoutFreeTypeBitmap { left: (*slot).bitmap_left, top: (*slot).bitmap_top,
                width: bitmap.width, rows: bitmap.rows, pitch: bitmap.pitch,
                gray: u32::from(bitmap.pixel_mode == ft::FT_PIXEL_MODE_GRAY), data: bitmap.buffer.cast() };
        }
    }
    1
}

unsafe extern "C" fn shape(ctx: *mut c_void, font: usize, text: *const u16, length: usize, start: usize, count: usize, out: *mut *const Hp2TextLayoutFreeTypeGlyph, out_count: *mut usize) -> i32 {
    if font == 0 || text.is_null() || out.is_null() || out_count.is_null() { return 0; }
    // SAFETY: font is a live hb_font and text describes length UTF-16 units.
    unsafe {
        let buffer = hb::hb_buffer_create();
        if buffer.is_null() { return 0; }
        if hb::hb_buffer_allocation_successful(buffer) == 0 { hb::hb_buffer_destroy(buffer); return 0; }
        hb::hb_buffer_add_utf16(buffer, text, length as c_int, start as u32, count as c_int);
        hb::hb_buffer_guess_segment_properties(buffer);
        hb::hb_shape(font as *mut hb::HbFont, buffer, core::ptr::null(), 0);
        if hb::hb_buffer_allocation_successful(buffer) == 0 { hb::hb_buffer_destroy(buffer); return 0; }
        let (mut ni, mut np) = (0, 0);
        let infos = hb::hb_buffer_get_glyph_infos(buffer, &mut ni);
        let positions = hb::hb_buffer_get_glyph_positions(buffer, &mut np);
        if ni != np || (ni != 0 && (infos.is_null() || positions.is_null())) { hb::hb_buffer_destroy(buffer); return 0; }
        let scratch = host(ctx);
        scratch.shaped.clear();
        scratch.shaped.reserve(ni as usize);
        for i in 0..ni as usize {
            let info = &*infos.add(i);
            let pos = &*positions.add(i);
            scratch.shaped.push(Hp2TextLayoutFreeTypeGlyph { glyph: info.codepoint, cluster: info.cluster,
                x_advance: pos.x_advance, y_advance: pos.y_advance, x_offset: pos.x_offset, y_offset: pos.y_offset });
        }
        *out = scratch.shaped.as_ptr();
        *out_count = scratch.shaped.len();
        hb::hb_buffer_destroy(buffer);
        1
    }
}

unsafe extern "C" fn resolve(ctx: *mut c_void, family: *const u8, length: usize, bold: u32, point: f64, scalar: u32, require_scalar: u32, out_path: *mut *const u8, out_length: *mut usize, out_index: *mut i32) -> i32 {
    if family.is_null() || out_path.is_null() || out_length.is_null() || out_index.is_null() { return 0; }
    // SAFETY: pointers are valid by callback contract; Fontconfig objects are destroyed on every path.
    unsafe {
        let name = match std::ffi::CString::new(core::slice::from_raw_parts(family, length)) { Ok(n) => n, Err(_) => return 0 };
        let pattern = fc::FcPatternCreate();
        if pattern.is_null() { return 0; }
        if require_scalar != 0 {
            let charset = fc::FcCharSetCreate();
            if charset.is_null() || fc::FcCharSetAddChar(charset, scalar) == 0 {
                if !charset.is_null() { fc::FcCharSetDestroy(charset); }
                fc::FcPatternDestroy(pattern); return 0;
            }
            fc::FcPatternAddCharSet(pattern, fc_object(fc::FC_CHARSET), charset);
            fc::FcCharSetDestroy(charset);
        }
        fc::FcPatternAddString(pattern, fc_object(fc::FC_FAMILY), name.as_ptr().cast());
        fc::FcPatternAddInteger(pattern, fc_object(fc::FC_WEIGHT), if bold != 0 { fc::FC_WEIGHT_BOLD } else { fc::FC_WEIGHT_REGULAR });
        fc::FcPatternAddDouble(pattern, fc_object(fc::FC_SIZE), point);
        fc::FcConfigSubstitute(core::ptr::null_mut(), pattern, fc::FC_MATCH_PATTERN);
        fc::FcDefaultSubstitute(pattern);
        let mut result = fc::FC_RESULT_MATCH;
        let matched = fc::FcFontMatch(core::ptr::null_mut(), pattern, &mut result);
        fc::FcPatternDestroy(pattern);
        if matched.is_null() { return 0; }
        let mut success = if require_scalar != 0 {
            let mut charset = core::ptr::null_mut();
            fc::FcPatternGetCharSet(matched, fc_object(fc::FC_CHARSET), 0, &mut charset) == fc::FC_RESULT_MATCH && !charset.is_null() && fc::FcCharSetHasChar(charset, scalar) != 0
        } else { true };
        let mut file = core::ptr::null_mut();
        if success && fc::FcPatternGetString(matched, fc_object(fc::FC_FILE), 0, &mut file) == fc::FC_RESULT_MATCH && !file.is_null() {
            let bytes = CStr::from_ptr(file.cast()).to_bytes();
            let scratch = host(ctx);
            scratch.resolved_path.clear(); scratch.resolved_path.extend_from_slice(bytes);
            let mut index = 0;
            fc::FcPatternGetInteger(matched, fc_object(fc::FC_INDEX), 0, &mut index);
            *out_path = scratch.resolved_path.as_ptr(); *out_length = scratch.resolved_path.len(); *out_index = index;
            success = true;
        } else { success = false; }
        fc::FcPatternDestroy(matched);
        i32::from(success)
    }
}

unsafe fn fc_object(name: &'static [u8]) -> *const c_char { name.as_ptr().cast() }

unsafe extern "C" fn page(ctx: *mut c_void, index: usize, data: *mut *const u8, count: *mut usize, stride: *mut usize, offset: *mut usize) -> i32 {
    if data.is_null() || count.is_null() || stride.is_null() || offset.is_null() { return 0; }
    // SAFETY: all outputs are valid pointers; context points to the live owner.
    unsafe {
        let Some(entry) = host(ctx).pages.get(index) else { return 0; };
        *data = entry.data.as_ptr(); *count = entry.count; *stride = entry.stride; *offset = entry.offset;
    }
    1
}

unsafe extern "C" fn alpha_buffer(ctx: *mut c_void, width: i32, height: i32, length: usize, out: *mut *mut u8) -> i32 {
    if out.is_null() || width <= 0 || height <= 0 || (width as usize).checked_mul(height as usize) != Some(length) { return 0; }
    // SAFETY: out is writable; host owns the allocation for its entire lifetime.
    unsafe {
        let owner = host(ctx);
        owner.alpha.resize(length, 0);
        *out = owner.alpha.as_mut_ptr();
    }
    1
}

