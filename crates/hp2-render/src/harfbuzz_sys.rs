//! Hand-written HarfBuzz FFI bindings: only the `hb_*` entry points and
//! struct fields the NativeText FreeType provider leaf's `Shape()` used
//! before this port. Pinned version: `ThirdParty/sources.json` HarfBuzz
//! `14.5.1`. No crates.io HarfBuzz binding is used; this binds the same
//! pinned static library the surviving C++ leaf links, the way
//! `freetype_sys` binds pinned FreeType.
//!
//! `hb_glyph_info_t`/`hb_glyph_position_t` layouts below were confirmed
//! against the real HarfBuzz headers (`hb-buffer.h`, `hb-common.h`): every
//! field HarfBuzz itself documents as `/*< public >*/` is 4 bytes
//! (`hb_codepoint_t`/`hb_mask_t` = `uint32_t`, `hb_position_t` = `int32_t`,
//! `hb_var_int_t` = 4-byte union), so both structs are 20 bytes with the
//! fields this module reads at the offsets asserted below.
//! `NativeTextFreeType.cpp` carries matching `static_assert`s against the
//! real headers so a future HarfBuzz upgrade that shifts these fields fails
//! the C++ build instead of silently corrupting reads.
#![cfg(feature = "freetype")]

use core::ffi::{c_int, c_uint, c_void};

/// Opaque `hb_font_t*`/`hb_buffer_t*`: never dereferenced by Rust, only
/// passed back into `hb_*` calls (ownership stays with the C++ leaf, which
/// still creates the `hb_font_t` via `hb_ft_font_create_referenced` and
/// destroys it with `hb_font_destroy`).
pub type HbFont = c_void;
pub type HbBuffer = c_void;

/// `hb_glyph_info_t` (`hb-buffer.h`): `codepoint` at offset 0, `cluster` at
/// offset 8; `mask`/`var1`/`var2` are private fields kept only to preserve
/// the real struct's 20-byte size and field offsets.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HbGlyphInfo {
    pub codepoint: u32,
    pub mask: u32,
    pub cluster: u32,
    pub var1: u32,
    pub var2: u32,
}

/// `hb_glyph_position_t` (`hb-buffer.h`): all four public fields kept in
/// order, plus the trailing private `var` to preserve the real 20-byte
/// size.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HbGlyphPosition {
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub var: u32,
}

// No unconditional `#[link(name = "...")]`: like `freetype_sys`, these
// symbols are resolved when CMake links the final `hp2_game` executable
// against the already-pinned, already-linked static HarfBuzz library (see
// `HP2Dependencies.cmake`), not by a Cargo-level link directive.
unsafe extern "C" {
    pub fn hb_buffer_create() -> *mut HbBuffer;
    pub fn hb_buffer_destroy(buffer: *mut HbBuffer);
    pub fn hb_buffer_allocation_successful(buffer: *mut HbBuffer) -> c_int;
    pub fn hb_buffer_add_utf16(
        buffer: *mut HbBuffer,
        text: *const u16,
        text_length: c_int,
        item_offset: c_uint,
        item_length: c_int,
    );
    pub fn hb_buffer_guess_segment_properties(buffer: *mut HbBuffer);
    pub fn hb_shape(
        font: *mut HbFont,
        buffer: *mut HbBuffer,
        features: *const c_void,
        num_features: c_uint,
    );
    pub fn hb_buffer_get_glyph_infos(
        buffer: *mut HbBuffer,
        length: *mut c_uint,
    ) -> *mut HbGlyphInfo;
    pub fn hb_buffer_get_glyph_positions(
        buffer: *mut HbBuffer,
        length: *mut c_uint,
    ) -> *mut HbGlyphPosition;
    // Test-oracle only: production `hb_font_t` creation stays in the C++
    // leaf's `Open()`, which owns `FNativeTextFace`'s FreeType+HarfBuzz pair
    // together for the lifetime of one loaded face.
    pub fn hb_ft_font_create_referenced(ft_face: *mut c_void) -> *mut HbFont;
    pub fn hb_font_destroy(font: *mut HbFont);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ground truth below was captured from the pinned HarfBuzz 14.5.x
    // headers (`hb-buffer.h`, `hb-common.h`): `hb_codepoint_t`/`hb_mask_t`
    // are `uint32_t`, `hb_position_t` is `int32_t`, `hb_var_int_t` is a
    // 4-byte union, so each struct is exactly 5 4-byte fields.
    #[test]
    fn glyph_info_matches_pinned_harfbuzz() {
        assert_eq!(core::mem::size_of::<HbGlyphInfo>(), 20);
        assert_eq!(core::mem::offset_of!(HbGlyphInfo, codepoint), 0);
        assert_eq!(core::mem::offset_of!(HbGlyphInfo, cluster), 8);
    }

    #[test]
    fn glyph_position_matches_pinned_harfbuzz() {
        assert_eq!(core::mem::size_of::<HbGlyphPosition>(), 20);
        assert_eq!(core::mem::offset_of!(HbGlyphPosition, x_advance), 0);
        assert_eq!(core::mem::offset_of!(HbGlyphPosition, y_advance), 4);
        assert_eq!(core::mem::offset_of!(HbGlyphPosition, x_offset), 8);
        assert_eq!(core::mem::offset_of!(HbGlyphPosition, y_offset), 12);
    }
}
