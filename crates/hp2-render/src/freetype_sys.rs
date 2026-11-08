//! Hand-written FreeType FFI bindings: only the `FT_*` entry points and
//! struct fields the NativeText FreeType provider leaf
//! (`NativeText/Src/NativeTextFreeType.cpp`) reads. Pinned version:
//! `ThirdParty/sources.json` FreeType `VER-2-14-3`. No crates.io FreeType
//! binding is used; this binds the same pinned static library the surviving
//! C++ leaf links, the way `hp2-platform::input_pump` binds SDL2.
//!
//! Struct field layouts below are **prefixes** of the real FreeType structs
//! (same field order/types up to the last field this module reads; trailing
//! fields such as `FT_FaceRec::driver` are never represented). A `repr(C)`
//! prefix has identical offsets to the real struct for every field it keeps,
//! so this is sound without mirroring fields this module never touches.
//!
//! Every offset/size here was computed from a standalone C translation unit
//! built against the pinned FreeType headers (`offsetof`/`sizeof`), not
//! hand-derived; `NativeTextFreeType.cpp` carries matching `static_assert`s
//! against the real headers (`freetype.h`/`ftimage.h`) so any future
//! FreeType upgrade that shifts these fields fails the C++ build instead of
//! silently corrupting reads.
#![cfg(feature = "freetype")]

use core::ffi::{c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void};

pub type FtPos = c_long;
pub type FtFixed = c_long;

/// `FT_Generic` (`fttypes.h:470-475`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FtGeneric {
    pub data: *mut c_void,
    pub finalizer: *mut c_void,
}

/// `FT_BBox` (`ftimage.h:118-123`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FtBBox {
    pub x_min: FtPos,
    pub y_min: FtPos,
    pub x_max: FtPos,
    pub y_max: FtPos,
}

/// `FT_Glyph_Metrics` (`freetype.h:444-457`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FtGlyphMetrics {
    pub width: FtPos,
    pub height: FtPos,
    pub hori_bearing_x: FtPos,
    pub hori_bearing_y: FtPos,
    pub hori_advance: FtPos,
    pub vert_bearing_x: FtPos,
    pub vert_bearing_y: FtPos,
    pub vert_advance: FtPos,
}

/// `FT_Vector` (`ftimage.h:75-80`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FtVector {
    pub x: FtPos,
    pub y: FtPos,
}

/// `FT_Size_Metrics` (`freetype.h:1974-1987`), mirrored in full.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FtSizeMetrics {
    pub x_ppem: c_ushort,
    pub y_ppem: c_ushort,
    pub x_scale: FtFixed,
    pub y_scale: FtFixed,
    pub ascender: FtPos,
    pub descender: FtPos,
    pub height: FtPos,
    pub max_advance: FtPos,
}

/// Prefix of `FT_SizeRec` (`freetype.h:2011-2018`) through `metrics`; the
/// trailing `internal` field is never read.
#[repr(C)]
pub struct FtSizeRecPrefix {
    pub face: *mut c_void,
    pub generic: FtGeneric,
    pub metrics: FtSizeMetrics,
}

/// Prefix of `FT_FaceRec` (`freetype.h:1229-1283`) through `size`; later
/// fields (`charmap`, `driver`, `memory`, ...) are never read.
#[repr(C)]
pub struct FtFaceRecPrefix {
    pub num_faces: c_long,
    pub face_index: c_long,
    pub face_flags: c_long,
    pub style_flags: c_long,
    pub num_glyphs: c_long,
    pub family_name: *mut c_char,
    pub style_name: *mut c_char,
    pub num_fixed_sizes: c_int,
    pub available_sizes: *mut c_void,
    pub num_charmaps: c_int,
    pub charmaps: *mut c_void,
    pub generic: FtGeneric,
    pub bbox: FtBBox,
    pub units_per_em: c_ushort,
    pub ascender: c_short,
    pub descender: c_short,
    pub height: c_short,
    pub max_advance_width: c_short,
    pub max_advance_height: c_short,
    pub underline_position: c_short,
    pub underline_thickness: c_short,
    pub glyph: *mut FtGlyphSlotRecPrefix,
    pub size: *mut FtSizeRecPrefix,
}

/// `FT_Bitmap` (`ftimage.h:275-286`), mirrored in full.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FtBitmap {
    pub rows: c_uint,
    pub width: c_uint,
    pub pitch: c_int,
    pub buffer: *mut c_uchar,
    pub num_grays: c_ushort,
    pub pixel_mode: c_uchar,
    pub palette_mode: c_uchar,
    pub palette: *mut c_void,
}

/// Prefix of `FT_GlyphSlotRec` (`freetype.h:2264-2298`) through
/// `bitmap_top`; later fields (`outline`, `subglyphs`, ...) are never read.
#[repr(C)]
pub struct FtGlyphSlotRecPrefix {
    pub library: *mut c_void,
    pub face: *mut c_void,
    pub next: *mut c_void,
    pub glyph_index: c_uint,
    pub generic: FtGeneric,
    pub metrics: FtGlyphMetrics,
    pub linear_hori_advance: FtFixed,
    pub linear_vert_advance: FtFixed,
    pub advance: FtVector,
    pub format: c_int,
    pub bitmap: FtBitmap,
    pub bitmap_left: c_int,
    pub bitmap_top: c_int,
}

pub const FT_LOAD_DEFAULT: c_int = 0x0;
pub const FT_LOAD_RENDER: c_int = 0x4;
pub const FT_PIXEL_MODE_GRAY: c_uchar = 2;

// No unconditional `#[link(name = "...")]`: like `hp2-platform::input_pump`'s
// SDL_* and `hp2-audio`'s AL*/alc* bindings, these symbols are resolved when
// CMake links the final `hp2_game` executable against the already-pinned,
// already-linked static FreeType library (see `HP2Dependencies.cmake`), not
// by a Cargo-level link directive.
unsafe extern "C" {
    pub fn FT_Init_FreeType(alibrary: *mut *mut c_void) -> c_int;
    pub fn FT_Done_FreeType(library: *mut c_void) -> c_int;
    pub fn FT_New_Face(
        library: *mut c_void,
        filepathname: *const c_char,
        face_index: c_long,
        aface: *mut *mut FtFaceRecPrefix,
    ) -> c_int;
    pub fn FT_Done_Face(face: *mut FtFaceRecPrefix) -> c_int;
    pub fn FT_Set_Char_Size(
        face: *mut FtFaceRecPrefix,
        char_width: c_long,
        char_height: c_long,
        horz_resolution: c_uint,
        vert_resolution: c_uint,
    ) -> c_int;
    pub fn FT_Get_Char_Index(face: *mut FtFaceRecPrefix, charcode: c_ulong) -> c_uint;
    pub fn FT_Load_Glyph(
        face: *mut FtFaceRecPrefix,
        glyph_index: c_uint,
        load_flags: c_int,
    ) -> c_int;
    pub fn FT_MulFix(a: FtFixed, b: FtFixed) -> FtFixed;
}

#[cfg(test)]
mod tests {
    use super::*;
    // Ground truth below was captured by compiling a standalone C program
    // against the pinned FreeType `VER-2-14-3` headers
    // (`freetype/freetype.h`, `freetype/ftimage.h`) and printing
    // `sizeof`/`offsetof`; `NativeTextFreeType.cpp` asserts the same
    // constants against the real headers at C++ compile time.
    #[test]
    fn face_rec_prefix_matches_pinned_freetype() {
        assert_eq!(
            core::mem::offset_of!(FtFaceRecPrefix, underline_position),
            148
        );
        assert_eq!(
            core::mem::offset_of!(FtFaceRecPrefix, underline_thickness),
            150
        );
        assert_eq!(core::mem::offset_of!(FtFaceRecPrefix, glyph), 152);
        assert_eq!(core::mem::offset_of!(FtFaceRecPrefix, size), 160);
        assert_eq!(core::mem::size_of::<FtFaceRecPrefix>(), 168);
    }
    #[test]
    fn size_rec_prefix_matches_pinned_freetype() {
        assert_eq!(core::mem::offset_of!(FtSizeRecPrefix, metrics), 24);
        assert_eq!(core::mem::size_of::<FtSizeRecPrefix>(), 80);
    }
    #[test]
    fn size_metrics_matches_pinned_freetype() {
        assert_eq!(core::mem::offset_of!(FtSizeMetrics, y_scale), 16);
        assert_eq!(core::mem::offset_of!(FtSizeMetrics, ascender), 24);
        assert_eq!(core::mem::offset_of!(FtSizeMetrics, descender), 32);
        assert_eq!(core::mem::offset_of!(FtSizeMetrics, height), 40);
        assert_eq!(core::mem::size_of::<FtSizeMetrics>(), 56);
    }
    #[test]
    fn glyph_slot_rec_prefix_matches_pinned_freetype() {
        assert_eq!(core::mem::offset_of!(FtGlyphSlotRecPrefix, bitmap), 152);
        assert_eq!(
            core::mem::offset_of!(FtGlyphSlotRecPrefix, bitmap_left),
            192
        );
        assert_eq!(core::mem::offset_of!(FtGlyphSlotRecPrefix, bitmap_top), 196);
        assert_eq!(core::mem::size_of::<FtGlyphSlotRecPrefix>(), 200);
    }
    #[test]
    fn bitmap_matches_pinned_freetype() {
        assert_eq!(core::mem::offset_of!(FtBitmap, rows), 0);
        assert_eq!(core::mem::offset_of!(FtBitmap, width), 4);
        assert_eq!(core::mem::offset_of!(FtBitmap, pitch), 8);
        assert_eq!(core::mem::offset_of!(FtBitmap, buffer), 16);
        assert_eq!(core::mem::offset_of!(FtBitmap, pixel_mode), 26);
        assert_eq!(core::mem::size_of::<FtBitmap>(), 40);
    }
}
